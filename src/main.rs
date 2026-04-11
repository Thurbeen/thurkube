use anyhow::Result;
use clap::Parser;
use kube::CustomResourceExt;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

use thurkube::crd::{
    AgentAuth, AgentJob, AgentRole, AgentRuntime, AgentSkill, ClusterAccess, McpServer, Repository,
};

const VERSION: &str = env!("THURKUBE_VERSION");

#[derive(Parser)]
#[command(name = "thurkube", version = VERSION)]
struct Cli {
    /// Print all CRD definitions as YAML and exit.
    #[arg(long)]
    crd: bool,
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
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();

    info!(version = VERSION, "starting thurkube controller");

    Ok(())
}
