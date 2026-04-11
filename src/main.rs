use anyhow::Result;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

const VERSION: &str = env!("THURKUBE_VERSION");

#[tokio::main]
async fn main() -> Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();

    info!(version = VERSION, "starting thurkube controller");

    Ok(())
}
