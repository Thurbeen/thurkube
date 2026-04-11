use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Defines how an agent runs: container image, entrypoint,
/// and path conventions for config/persistence mounts.
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "thurkube.thurbeen.eu",
    version = "v1alpha1",
    kind = "AgentRuntime",
    namespaced,
    shortname = "ar"
)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeSpec {
    /// Container image for the agent.
    pub image: String,

    /// Container command override.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,

    /// Name of the environment variable used to inject the auth
    /// token (e.g. "CLAUDE_CODE_OAUTH_TOKEN").
    pub auth_env_var: String,

    /// Path where the generated ConfigMap is mounted (read-only).
    pub config_path: String,

    /// Path where the PVC is mounted when persistence is enabled.
    pub persist_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::CustomResourceExt;

    #[test]
    fn crd_generation() {
        let crd = AgentRuntime::crd();
        assert_eq!(
            crd.metadata.name.unwrap(),
            "agentruntimes.thurkube.thurbeen.eu"
        );
    }

    #[test]
    fn deserialize_full() {
        let yaml = r#"
            image: "ghcr.io/thurbeen/claude-code-job:latest"
            command: ["/usr/local/bin/entrypoint.sh"]
            authEnvVar: CLAUDE_CODE_OAUTH_TOKEN
            configPath: /etc/claude-code-job
            persistPath: /var/lib/claude-code-job
        "#;
        let spec: AgentRuntimeSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.auth_env_var, "CLAUDE_CODE_OAUTH_TOKEN");
        assert_eq!(spec.command, vec!["/usr/local/bin/entrypoint.sh"]);
    }

    #[test]
    fn deserialize_minimal() {
        let yaml = r#"
            image: "ghcr.io/thurbeen/agent:latest"
            authEnvVar: TOKEN
            configPath: /config
            persistPath: /data
        "#;
        let spec: AgentRuntimeSpec = serde_yaml::from_str(yaml).unwrap();
        assert!(spec.command.is_empty());
    }
}
