use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// References a Kubernetes Secret key that holds the agent's
/// authentication token.
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "thurkube.thurbeen.eu",
    version = "v1alpha1",
    kind = "AgentAuth",
    namespaced,
    shortname = "aa"
)]
#[serde(rename_all = "camelCase")]
pub struct AgentAuthSpec {
    /// Reference to a Secret key containing the auth token.
    pub secret_ref: SecretKeyRef,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretKeyRef {
    /// Name of the Secret in the same namespace.
    pub name: String,

    /// Key within the Secret.
    pub key: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::CustomResourceExt;

    #[test]
    fn crd_generation() {
        let crd = AgentAuth::crd();
        assert_eq!(
            crd.metadata.name.unwrap(),
            "agentauths.thurkube.thurbeen.eu"
        );
    }

    #[test]
    fn deserialize() {
        let yaml = r#"
            secretRef:
              name: claude-code-jobs
              key: CLAUDE_CODE_OAUTH_TOKEN
        "#;
        let spec: AgentAuthSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.secret_ref.name, "claude-code-jobs");
        assert_eq!(spec.secret_ref.key, "CLAUDE_CODE_OAUTH_TOKEN");
    }
}
