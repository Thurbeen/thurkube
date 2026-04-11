use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::agent_auth::SecretKeyRef;

/// Defines a GitHub repository with its access token.
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "thurkube.thurbeen.eu",
    version = "v1alpha1",
    kind = "Repository",
    namespaced,
    shortname = "repo"
)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySpec {
    /// GitHub organization or user.
    pub owner: String,

    /// Repository name.
    pub name: String,

    /// Reference to a Secret key containing the GitHub token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_secret_ref: Option<SecretKeyRef>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::CustomResourceExt;

    #[test]
    fn crd_generation() {
        let crd = Repository::crd();
        assert_eq!(
            crd.metadata.name.unwrap(),
            "repositories.thurkube.thurbeen.eu"
        );
    }

    #[test]
    fn deserialize_with_token() {
        let yaml = r#"
            owner: Thurbeen
            name: thurspace
            tokenSecretRef:
              name: github-tokens
              key: GH_TOKEN
        "#;
        let spec: RepositorySpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.owner, "Thurbeen");
        assert_eq!(spec.name, "thurspace");
        let token_ref = spec.token_secret_ref.unwrap();
        assert_eq!(token_ref.name, "github-tokens");
        assert_eq!(token_ref.key, "GH_TOKEN");
    }

    #[test]
    fn deserialize_without_token() {
        let yaml = r#"
            owner: Thurbeen
            name: thurbox
        "#;
        let spec: RepositorySpec = serde_yaml::from_str(yaml).unwrap();
        assert!(spec.token_secret_ref.is_none());
    }
}
