use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Defines which tools an agent is allowed to use.
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "thurkube.thurbeen.eu",
    version = "v1alpha1",
    kind = "AgentRole",
    namespaced,
    shortname = "arl"
)]
#[serde(rename_all = "camelCase")]
pub struct AgentRoleSpec {
    /// List of allowed tool names (e.g. "Bash", "Read",
    /// "mcp__gmail__*").
    pub allowed_tools: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::CustomResourceExt;

    #[test]
    fn crd_generation() {
        let crd = AgentRole::crd();
        assert_eq!(
            crd.metadata.name.unwrap(),
            "agentroles.thurkube.thurbeen.eu"
        );
    }

    #[test]
    fn deserialize() {
        let yaml = r#"
            allowedTools:
              - Bash
              - Read
              - "mcp__gmail__*"
        "#;
        let spec: AgentRoleSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.allowed_tools.len(), 3);
        assert_eq!(spec.allowed_tools[2], "mcp__gmail__*");
    }
}
