use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Defines an MCP server — either a local command or a remote
/// HTTP endpoint.
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "thurkube.thurbeen.eu",
    version = "v1alpha1",
    kind = "McpServer",
    namespaced,
    shortname = "mcp"
)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSpec {
    /// Command to start a local MCP server (e.g. "npx").
    /// Mutually exclusive with `url`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Arguments for the command.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,

    /// URL of a remote MCP server.
    /// Mutually exclusive with `command`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::CustomResourceExt;

    #[test]
    fn crd_generation() {
        let crd = McpServer::crd();
        assert_eq!(
            crd.metadata.name.unwrap(),
            "mcpservers.thurkube.thurbeen.eu"
        );
    }

    #[test]
    fn deserialize_command_based() {
        let yaml = r#"
            command: npx
            args: ["-y", "gmail-mcp"]
        "#;
        let spec: McpServerSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.command.unwrap(), "npx");
        assert_eq!(spec.args, vec!["-y", "gmail-mcp"]);
        assert!(spec.url.is_none());
    }

    #[test]
    fn deserialize_url_based() {
        let yaml = r#"
            url: "https://dev.helloasso.com/mcp"
        "#;
        let spec: McpServerSpec = serde_yaml::from_str(yaml).unwrap();
        assert!(spec.command.is_none());
        assert_eq!(spec.url.unwrap(), "https://dev.helloasso.com/mcp");
    }
}
