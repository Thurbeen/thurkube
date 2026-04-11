use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Defines a reusable skill from a GitHub repository.
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "thurkube.thurbeen.eu",
    version = "v1alpha1",
    kind = "AgentSkill",
    namespaced,
    shortname = "ask"
)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkillSpec {
    /// GitHub repository in "owner/repo" format.
    pub repo: String,

    /// Skill directory name within the repository.
    pub name: String,

    /// Git ref to checkout. Defaults to "main".
    #[serde(default = "default_ref")]
    pub r#ref: String,
}

fn default_ref() -> String {
    "main".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::CustomResourceExt;

    #[test]
    fn crd_generation() {
        let crd = AgentSkill::crd();
        assert_eq!(
            crd.metadata.name.unwrap(),
            "agentskills.thurkube.thurbeen.eu"
        );
    }

    #[test]
    fn deserialize_with_default_ref() {
        let yaml = r#"
            repo: Thurbeen/thurbeen-skills
            name: bump-pr-fixer
        "#;
        let spec: AgentSkillSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.repo, "Thurbeen/thurbeen-skills");
        assert_eq!(spec.name, "bump-pr-fixer");
        assert_eq!(spec.r#ref, "main");
    }

    #[test]
    fn deserialize_with_explicit_ref() {
        let yaml = r#"
            repo: Thurbeen/thurbeen-skills
            name: infra-monitor
            ref: v1.2.0
        "#;
        let spec: AgentSkillSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.r#ref, "v1.2.0");
    }
}
