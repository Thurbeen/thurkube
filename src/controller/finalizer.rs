//! Finalizer handling — cleanup cluster-scoped RBAC on delete.

use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding};
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::Api;
use kube::{Client, ResourceExt};

use crate::controller::build::owner_label_selector;
use crate::controller::{Error, FINALIZER};
use crate::crd::AgentJob;

pub fn has_finalizer(ajob: &AgentJob) -> bool {
    ajob.metadata
        .finalizers
        .as_ref()
        .is_some_and(|f| f.iter().any(|x| x == FINALIZER))
}

pub async fn add_finalizer(api: &Api<AgentJob>, ajob: &AgentJob) -> Result<(), Error> {
    if has_finalizer(ajob) {
        return Ok(());
    }
    let name = ajob.name_any();
    let mut finalizers = ajob.metadata.finalizers.clone().unwrap_or_default();
    finalizers.push(FINALIZER.into());
    let patch = serde_json::json!({
        "metadata": { "finalizers": finalizers }
    });
    api.patch(&name, &PatchParams::default(), &Patch::Merge(&patch))
        .await?;
    Ok(())
}

pub async fn remove_finalizer(api: &Api<AgentJob>, ajob: &AgentJob) -> Result<(), Error> {
    if !has_finalizer(ajob) {
        return Ok(());
    }
    let name = ajob.name_any();
    let finalizers: Vec<String> = ajob
        .metadata
        .finalizers
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|f| f != FINALIZER)
        .collect();
    let patch = serde_json::json!({
        "metadata": { "finalizers": finalizers }
    });
    api.patch(&name, &PatchParams::default(), &Patch::Merge(&patch))
        .await?;
    Ok(())
}

pub async fn cleanup_cluster_scoped(client: &Client, ajob: &AgentJob) -> Result<(), Error> {
    let Some(selector) = owner_label_selector(ajob) else {
        return Ok(());
    };
    let lp = ListParams::default().labels(&selector);
    let dp = DeleteParams::background();

    let crs: Api<ClusterRole> = Api::all(client.clone());
    for cr in crs.list(&lp).await?.items {
        let name = cr.name_any();
        let _ = crs.delete(&name, &dp).await;
    }
    let crbs: Api<ClusterRoleBinding> = Api::all(client.clone());
    for crb in crbs.list(&lp).await?.items {
        let name = crb.name_any();
        let _ = crbs.delete(&name, &dp).await;
    }
    Ok(())
}
