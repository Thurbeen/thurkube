//! AgentJob reconciler.

use std::sync::Arc;

use chrono::Utc;
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{ConfigMap, PersistentVolumeClaim, ServiceAccount};
use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding};
use kube::api::{Api, DeleteParams, Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::runtime::events::{Event, EventType};
use kube::{Resource, ResourceExt};
use tracing::{info, warn};

use crate::controller::context::Ctx;
use crate::controller::status::{patch_status, StatusUpdate};
use crate::controller::{
    build, finalizer, requeue_short, requeue_steady, resolve, Error, FIELD_MANAGER,
};
use crate::crd::{AgentJob, OwnedResource};

pub async fn reconcile(ajob: Arc<AgentJob>, ctx: Arc<Ctx>) -> Result<Action, Error> {
    let ns = ajob
        .metadata
        .namespace
        .clone()
        .ok_or_else(|| Error::InvalidSpec("AgentJob missing namespace".into()))?;
    let api: Api<AgentJob> = Api::namespaced(ctx.client.clone(), &ns);

    if ajob.metadata.deletion_timestamp.is_some() {
        info!(name = %ajob.name_any(), "handling AgentJob deletion");
        finalizer::cleanup_cluster_scoped(&ctx.client, &ajob).await?;
        finalizer::remove_finalizer(&api, &ajob).await?;
        return Ok(Action::await_change());
    }

    finalizer::add_finalizer(&api, &ajob).await?;

    if ajob.spec.suspend {
        info!(name = %ajob.name_any(), "AgentJob suspended");
        tear_down_workloads(&ctx.client, &ns, &ajob).await;
        patch_status(
            &api,
            &ajob,
            StatusUpdate {
                phase: "Suspended",
                message: Some("spec.suspend=true".into()),
                ready: Some((false, "Suspended", "AgentJob suspended".into())),
                ..StatusUpdate::pending()
            },
        )
        .await?;
        return Ok(requeue_steady());
    }

    let resolved = match resolve::resolve(&ctx.client, &ajob).await {
        Ok(r) => r,
        Err(e @ Error::RefNotFound { .. }) => {
            warn!(error = %e, "reference not found");
            let _ = patch_status(
                &api,
                &ajob,
                StatusUpdate {
                    phase: "Pending",
                    message: Some(e.to_string()),
                    ready: Some((false, "ResolveFailed", e.to_string())),
                    ..StatusUpdate::pending()
                },
            )
            .await;
            let _ = ctx
                .recorder
                .publish(
                    &Event {
                        type_: EventType::Warning,
                        reason: "ResolveFailed".into(),
                        note: Some(e.to_string()),
                        action: "Resolve".into(),
                        secondary: None,
                    },
                    &ajob.object_ref(&()),
                )
                .await;
            return Ok(requeue_short());
        }
        Err(e) => return Err(e),
    };

    let agent_json = build::agent_config_json(&ajob, &resolved)?;
    let hash = build::config_hash(&agent_json);
    let hash_changed =
        ajob.status.as_ref().and_then(|s| s.config_hash.as_deref()) != Some(hash.as_str());

    let cm = build::configmap(&ajob, &ns, &agent_json);
    let mut owned: Vec<OwnedResource> = Vec::with_capacity(6);

    apply(
        &Api::<ConfigMap>::namespaced(ctx.client.clone(), &ns),
        &build::config_name(&ajob),
        &cm,
    )
    .await?;
    owned.push(OwnedResource {
        kind: "ConfigMap".into(),
        name: build::config_name(&ajob),
    });

    if ajob.spec.persist {
        let pvc = build::pvc(&ajob, &ns);
        apply(
            &Api::<PersistentVolumeClaim>::namespaced(ctx.client.clone(), &ns),
            &build::pvc_name(&ajob),
            &pvc,
        )
        .await?;
        owned.push(OwnedResource {
            kind: "PersistentVolumeClaim".into(),
            name: build::pvc_name(&ajob),
        });
    }

    if ajob.spec.cluster_access_ref.is_some() {
        let sa = build::service_account(&ajob, &ns);
        apply(
            &Api::<ServiceAccount>::namespaced(ctx.client.clone(), &ns),
            &build::runner_name(&ajob),
            &sa,
        )
        .await?;
        owned.push(OwnedResource {
            kind: "ServiceAccount".into(),
            name: build::runner_name(&ajob),
        });

        let cr = build::cluster_role(&ajob, &resolved);
        apply(
            &Api::<ClusterRole>::all(ctx.client.clone()),
            &build::cluster_rbac_name(&ajob),
            &cr,
        )
        .await?;
        owned.push(OwnedResource {
            kind: "ClusterRole".into(),
            name: build::cluster_rbac_name(&ajob),
        });

        let crb = build::cluster_role_binding(&ajob, &ns);
        apply(
            &Api::<ClusterRoleBinding>::all(ctx.client.clone()),
            &build::cluster_rbac_name(&ajob),
            &crb,
        )
        .await?;
        owned.push(OwnedResource {
            kind: "ClusterRoleBinding".into(),
            name: build::cluster_rbac_name(&ajob),
        });
    }

    let (phase, kind, name) = if let Some(schedule) = ajob.spec.schedule.as_deref() {
        let cj = build::cron_job(&ajob, &resolved, &ns, schedule);
        apply(
            &Api::<CronJob>::namespaced(ctx.client.clone(), &ns),
            &build::cron_name(&ajob),
            &cj,
        )
        .await?;
        ("Scheduled", "CronJob", build::cron_name(&ajob))
    } else {
        let j = build::job(&ajob, &resolved, &ns, &hash);
        apply(
            &Api::<Job>::namespaced(ctx.client.clone(), &ns),
            &build::job_name(&ajob, &hash),
            &j,
        )
        .await?;
        ("Running", "Job", build::job_name(&ajob, &hash))
    };
    owned.push(OwnedResource {
        kind: kind.into(),
        name: name.clone(),
    });

    patch_status(
        &api,
        &ajob,
        StatusUpdate {
            phase,
            message: Some(format!("reconciled {kind}/{name}")),
            config_hash: Some(hash),
            owned,
            ready: Some((true, "Reconciled", format!("{kind}/{name}"))),
            last_run_time: hash_changed.then(|| Utc::now().to_rfc3339()),
        },
    )
    .await?;

    let _ = ctx
        .recorder
        .publish(
            &Event {
                type_: EventType::Normal,
                reason: "Reconciled".into(),
                note: Some(format!("applied {kind}/{name}")),
                action: "Apply".into(),
                secondary: None,
            },
            &ajob.object_ref(&()),
        )
        .await;

    Ok(requeue_steady())
}

pub fn error_policy(_obj: Arc<AgentJob>, err: &Error, _ctx: Arc<Ctx>) -> Action {
    warn!(error = %err, "reconcile failed");
    requeue_short()
}

async fn apply<K>(api: &Api<K>, name: &str, obj: &K) -> Result<(), Error>
where
    K: Resource + Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    api.patch(
        name,
        &PatchParams::apply(FIELD_MANAGER).force(),
        &Patch::Apply(obj),
    )
    .await?;
    Ok(())
}

async fn tear_down_workloads(client: &kube::Client, ns: &str, ajob: &AgentJob) {
    let dp = DeleteParams::background();
    let cj: Api<CronJob> = Api::namespaced(client.clone(), ns);
    let _ = cj.delete(&build::cron_name(ajob), &dp).await;

    if let Some(sel) = build::owner_label_selector(ajob) {
        let lp = kube::api::ListParams::default().labels(&sel);
        let jobs: Api<Job> = Api::namespaced(client.clone(), ns);
        if let Ok(list) = jobs.list(&lp).await {
            for j in list.items {
                let _ = jobs.delete(&j.name_any(), &dp).await;
            }
        }
    }
}
