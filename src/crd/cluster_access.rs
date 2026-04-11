use k8s_openapi::api::rbac::v1::PolicyRule;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Defines Kubernetes RBAC rules. When referenced by an
/// AgentJob, the controller creates a ServiceAccount,
/// ClusterRole, and ClusterRoleBinding.
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "thurkube.thurbeen.eu",
    version = "v1alpha1",
    kind = "ClusterAccess",
    namespaced,
    shortname = "ca"
)]
#[serde(rename_all = "camelCase")]
pub struct ClusterAccessSpec {
    /// Kubernetes RBAC policy rules. Same syntax as a
    /// ClusterRole's rules field.
    pub rules: Vec<PolicyRule>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::CustomResourceExt;

    #[test]
    fn crd_generation() {
        let crd = ClusterAccess::crd();
        assert_eq!(
            crd.metadata.name.unwrap(),
            "clusteraccesses.thurkube.thurbeen.eu"
        );
    }

    #[test]
    fn deserialize() {
        let yaml = r#"
            rules:
              - apiGroups: [""]
                resources: ["pods", "events", "nodes"]
                verbs: ["get", "list"]
              - apiGroups: ["apps"]
                resources: ["deployments"]
                verbs: ["get", "list"]
        "#;
        let spec: ClusterAccessSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.rules.len(), 2);
        assert_eq!(
            spec.rules[0].resources.as_ref().unwrap(),
            &vec!["pods".to_owned(), "events".to_owned(), "nodes".to_owned()]
        );
    }
}
