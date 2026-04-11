use k8s_openapi::api::core::v1::{EnvFromSource, EnvVar, ResourceRequirements};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The main orchestration unit. Defines a scheduled or one-shot
/// agent job with references to runtime, auth, role, skills,
/// MCP servers, repositories, and cluster access.
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "thurkube.thurbeen.eu",
    version = "v1alpha1",
    kind = "AgentJob",
    namespaced,
    status = "AgentJobStatus",
    shortname = "aj",
    printcolumn = r#"{"name":"Schedule","type":"string","jsonPath":".spec.schedule"}"#,
    printcolumn = r#"{"name":"Suspended","type":"boolean","jsonPath":".spec.suspend"}"#,
    printcolumn = r#"{"name":"Phase","type":"string","jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Last Run","type":"date","jsonPath":".status.lastRunTime"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct AgentJobSpec {
    // ── Scheduling ──────────────────────────────────────────
    /// Cron schedule expression. Omit for a one-shot Job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,

    /// IANA timezone for the schedule.
    #[serde(default = "default_timezone")]
    pub timezone: String,

    /// Suspend future executions without deleting the resource.
    #[serde(default)]
    pub suspend: bool,

    // ── References (by name, same namespace) ────────────────
    /// Reference to an AgentRuntime resource.
    pub runtime_ref: String,

    /// Reference to an AgentAuth resource.
    pub auth_ref: String,

    /// Reference to an AgentRole resource.
    pub role_ref: String,

    /// Reference to an AgentSkill resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_ref: Option<String>,

    /// Reference to a ClusterAccess resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cluster_access_ref: Option<String>,

    /// References to McpServer resources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_server_refs: Vec<String>,

    /// References to Repository resources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repository_refs: Vec<String>,

    // ── Agent config ────────────────────────────────────────
    /// The prompt text sent to the agent.
    pub prompt: String,

    /// Additional instructions mounted as project context
    /// (e.g. CLAUDE.md for Claude Code).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    /// Model override (e.g. "sonnet", "opus").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    // ── Container ───────────────────────────────────────────
    /// Resource requests and limits for the container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceRequirements>,

    /// Job timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u32,

    // ── Environment ─────────────────────────────────────────
    /// Environment variables (native K8s EnvVar).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<EnvVar>,

    /// Bulk environment injection from Secrets/ConfigMaps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_from: Vec<EnvFromSource>,

    // ── Storage ─────────────────────────────────────────────
    /// Enable persistent storage. When true, the controller
    /// creates a PVC with sensible defaults.
    #[serde(default)]
    pub persist: bool,
}

fn default_timezone() -> String {
    "Etc/UTC".to_owned()
}

fn default_timeout() -> u32 {
    3600
}

// ── Status ──────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentJobStatus {
    /// Current phase: Pending, Running, Succeeded, Failed,
    /// Suspended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,

    /// Last time the job was scheduled (ISO 8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_time: Option<String>,

    /// Last time the controller observed a completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_completion_time: Option<String>,

    /// Human-readable message about current state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Generation the controller last reconciled.
    #[serde(default)]
    pub observed_generation: i64,

    /// Hash of the rendered ConfigMap data for drift detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_hash: Option<String>,

    /// Child resources the controller owns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owned_resources: Vec<OwnedResource>,

    /// Standard Kubernetes conditions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnedResource {
    pub kind: String,
    pub name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    pub r#type: String,
    /// "True", "False", or "Unknown".
    pub status: String,
    pub last_transition_time: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::CustomResourceExt;

    #[test]
    fn crd_generation() {
        let crd = AgentJob::crd();
        assert_eq!(crd.metadata.name.unwrap(), "agentjobs.thurkube.thurbeen.eu");
    }

    #[test]
    fn deserialize_minimal() {
        let yaml = r#"
            runtimeRef: claude-code
            authRef: claude-oauth
            roleRef: default
            prompt: "Do the thing."
        "#;
        let spec: AgentJobSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.runtime_ref, "claude-code");
        assert_eq!(spec.auth_ref, "claude-oauth");
        assert_eq!(spec.role_ref, "default");
        assert_eq!(spec.prompt, "Do the thing.");
        assert_eq!(spec.timezone, "Etc/UTC");
        assert_eq!(spec.timeout_seconds, 3600);
        assert!(!spec.suspend);
        assert!(!spec.persist);
        assert!(spec.schedule.is_none());
        assert!(spec.skill_ref.is_none());
        assert!(spec.cluster_access_ref.is_none());
        assert!(spec.mcp_server_refs.is_empty());
        assert!(spec.repository_refs.is_empty());
        assert!(spec.env.is_empty());
        assert!(spec.env_from.is_empty());
        assert!(spec.resources.is_none());
        assert!(spec.instructions.is_none());
        assert!(spec.model.is_none());
    }

    #[test]
    fn deserialize_full() {
        let yaml = r#"
            schedule: "0 */6 * * *"
            timezone: Europe/Paris
            suspend: false
            runtimeRef: claude-code
            authRef: claude-oauth
            roleRef: default
            skillRef: bump-pr-fixer
            clusterAccessRef: infra-reader
            mcpServerRefs: [gmail, google-sheets]
            repositoryRefs: [thurspace, thurbox]
            prompt: "Fix failing PRs."
            instructions: "Extra context here."
            model: sonnet
            timeoutSeconds: 7200
            env:
              - name: REPOS
                value: "Thurbeen/thurspace"
            persist: true
        "#;
        let spec: AgentJobSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.schedule.unwrap(), "0 */6 * * *");
        assert_eq!(spec.timezone, "Europe/Paris");
        assert_eq!(spec.skill_ref.unwrap(), "bump-pr-fixer");
        assert_eq!(spec.cluster_access_ref.unwrap(), "infra-reader");
        assert_eq!(spec.mcp_server_refs, vec!["gmail", "google-sheets"]);
        assert_eq!(spec.repository_refs, vec!["thurspace", "thurbox"]);
        assert_eq!(spec.model.unwrap(), "sonnet");
        assert_eq!(spec.timeout_seconds, 7200);
        assert_eq!(spec.env.len(), 1);
        assert!(spec.persist);
    }
}
