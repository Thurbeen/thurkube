//! AgentJob controller runtime.

pub mod agentjob;
pub mod build;
pub mod context;
pub mod finalizer;
pub mod resolve;
pub mod status;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{ConfigMap, PersistentVolumeClaim, ServiceAccount};
use kube::api::{Api, ListParams};
use kube::runtime::controller::Action;
use kube::runtime::events::{Recorder, Reporter};
use kube::runtime::watcher;
use kube::runtime::Controller;
use kube::Client;
use tracing::{error, info, warn};

use crate::controller::context::Ctx;
use crate::crd::AgentJob;

pub const FIELD_MANAGER: &str = "thurkube";
pub const FINALIZER: &str = "thurkube.thurbeen.eu/finalizer";
pub const LABEL_MANAGED_BY: &str = "app.kubernetes.io/managed-by";
pub const LABEL_AGENTJOB: &str = "thurkube.thurbeen.eu/agentjob";
pub const LABEL_AGENTJOB_NS: &str = "thurkube.thurbeen.eu/agentjob-namespace";
pub const LABEL_OWNER_UID: &str = "thurkube.thurbeen.eu/owner-uid";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("kube API error: {0}")]
    Kube(#[from] kube::Error),

    #[error("resource {kind}/{name} not found in namespace {namespace}")]
    RefNotFound {
        kind: &'static str,
        name: String,
        namespace: String,
    },

    #[error("invalid spec: {0}")]
    InvalidSpec(String),

    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub async fn run(
    client: Client,
    ready: Arc<AtomicBool>,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<()> {
    let agent_jobs: Api<AgentJob> = Api::all(client.clone());

    if let Err(e) = agent_jobs.list(&ListParams::default().limit(1)).await {
        error!(error = %e, "failed to list AgentJob — is the CRD installed?");
        return Err(e.into());
    }

    let reporter = Reporter {
        controller: "thurkube".into(),
        instance: std::env::var("POD_NAME").ok(),
    };
    let recorder = Recorder::new(client.clone(), reporter);
    let ctx = Arc::new(Ctx {
        client: client.clone(),
        recorder,
        ready: ready.clone(),
    });

    let jobs: Api<Job> = Api::all(client.clone());
    let cronjobs: Api<CronJob> = Api::all(client.clone());
    let configmaps: Api<ConfigMap> = Api::all(client.clone());
    let service_accounts: Api<ServiceAccount> = Api::all(client.clone());
    let pvcs: Api<PersistentVolumeClaim> = Api::all(client.clone());

    let wc = watcher::Config::default();

    info!("starting AgentJob controller");

    let controller = Controller::new(agent_jobs, wc.clone())
        .owns(jobs, wc.clone())
        .owns(cronjobs, wc.clone())
        .owns(configmaps, wc.clone())
        .owns(service_accounts, wc.clone())
        .owns(pvcs, wc)
        .graceful_shutdown_on(async move { cancel.cancelled().await })
        .run(agentjob::reconcile, agentjob::error_policy, ctx.clone())
        .for_each(|res| async move {
            match res {
                Ok((obj, _)) => {
                    info!(
                        agentjob = %obj.name,
                        namespace = ?obj.namespace,
                        "reconciled"
                    );
                }
                Err(e) => {
                    warn!(error = %e, "reconcile error");
                }
            }
        });

    ready.store(true, Ordering::SeqCst);

    controller.await;
    info!("controller stream ended");
    Ok(())
}

pub fn requeue_steady() -> Action {
    Action::requeue(Duration::from_secs(300))
}

pub fn requeue_short() -> Action {
    Action::requeue(Duration::from_secs(30))
}
