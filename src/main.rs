use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use kube::{Client, CustomResourceExt};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use tracing_subscriber::{fmt, EnvFilter};

use thurkube::controller;
use thurkube::crd::{
    AgentAuth, AgentJob, AgentRole, AgentRuntime, AgentSkill, ClusterAccess, McpServer, Repository,
};
use thurkube::health;

const VERSION: &str = env!("THURKUBE_VERSION");

#[derive(Parser)]
#[command(name = "thurkube", version = VERSION)]
struct Cli {
    /// Print all CRD definitions as YAML and exit.
    #[arg(long)]
    crd: bool,

    /// Bind address for the health/readiness HTTP server.
    #[arg(long, env = "THURKUBE_HEALTH_ADDR", default_value = "0.0.0.0:8080")]
    health_addr: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.crd {
        let crds = [
            serde_yaml::to_string(&AgentJob::crd())?,
            serde_yaml::to_string(&AgentRuntime::crd())?,
            serde_yaml::to_string(&AgentAuth::crd())?,
            serde_yaml::to_string(&AgentRole::crd())?,
            serde_yaml::to_string(&AgentSkill::crd())?,
            serde_yaml::to_string(&McpServer::crd())?,
            serde_yaml::to_string(&Repository::crd())?,
            serde_yaml::to_string(&ClusterAccess::crd())?,
        ];
        print!("{}", crds.join("---\n"));
        return Ok(());
    }

    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    info!(version = VERSION, "starting thurkube controller");

    let client = Client::try_default()
        .await
        .context("building kube client")?;

    let ready = Arc::new(AtomicBool::new(false));
    let cancel = CancellationToken::new();

    let health_task = tokio::spawn({
        let ready = ready.clone();
        let cancel = cancel.clone();
        let addr = cli.health_addr;
        async move { health::serve(addr, ready, cancel).await }
    });

    let controller_task = tokio::spawn({
        let client = client.clone();
        let ready = ready.clone();
        let cancel = cancel.clone();
        async move { controller::run(client, ready, cancel).await }
    });

    let shutdown = async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => info!("SIGINT received"),
                _ = term.recv() => info!("SIGTERM received"),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            info!("ctrl-c received");
        }
    };

    tokio::select! {
        _ = shutdown => {
            info!("shutting down");
            cancel.cancel();
        }
        res = controller_task => {
            match res {
                Ok(Ok(())) => warn!("controller task ended"),
                Ok(Err(e)) => warn!(error = %e, "controller task errored"),
                Err(e) => warn!(error = %e, "controller task panicked"),
            }
            cancel.cancel();
        }
        res = health_task => {
            match res {
                Ok(Ok(())) => warn!("health task ended"),
                Ok(Err(e)) => warn!(error = %e, "health task errored"),
                Err(e) => warn!(error = %e, "health task panicked"),
            }
            cancel.cancel();
        }
    }

    info!("thurkube exited");
    Ok(())
}
