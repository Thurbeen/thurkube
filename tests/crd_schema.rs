//! Validate that all CRD schemas are well-formed and can be
//! serialized to YAML.

use kube::CustomResourceExt;
use thurkube::crd::{
    AgentAuth, AgentJob, AgentRole, AgentRuntime, AgentSkill, ClusterAccess, McpServer, Repository,
};

fn assert_crd_valid<T: CustomResourceExt>(expected_group: &str, expected_kind: &str) {
    let crd = T::crd();
    let name = crd.metadata.name.as_deref().unwrap();
    assert!(
        name.ends_with(expected_group),
        "CRD name {name} must end with {expected_group}"
    );

    let spec = &crd.spec;
    assert_eq!(spec.group, "thurkube.thurbeen.eu");
    assert_eq!(spec.names.kind, expected_kind, "CRD kind mismatch");

    // Must serialize to valid YAML
    let yaml = serde_yaml::to_string(&crd).expect("CRD must serialize to YAML");
    assert!(yaml.contains("apiVersion: apiextensions.k8s.io/v1"));
    assert!(yaml.contains("kind: CustomResourceDefinition"));
}

#[test]
fn agent_job_crd() {
    assert_crd_valid::<AgentJob>("thurkube.thurbeen.eu", "AgentJob");
}

#[test]
fn agent_runtime_crd() {
    assert_crd_valid::<AgentRuntime>("thurkube.thurbeen.eu", "AgentRuntime");
}

#[test]
fn agent_auth_crd() {
    assert_crd_valid::<AgentAuth>("thurkube.thurbeen.eu", "AgentAuth");
}

#[test]
fn agent_role_crd() {
    assert_crd_valid::<AgentRole>("thurkube.thurbeen.eu", "AgentRole");
}

#[test]
fn agent_skill_crd() {
    assert_crd_valid::<AgentSkill>("thurkube.thurbeen.eu", "AgentSkill");
}

#[test]
fn mcp_server_crd() {
    assert_crd_valid::<McpServer>("thurkube.thurbeen.eu", "McpServer");
}

#[test]
fn repository_crd() {
    assert_crd_valid::<Repository>("thurkube.thurbeen.eu", "Repository");
}

#[test]
fn cluster_access_crd() {
    assert_crd_valid::<ClusterAccess>("thurkube.thurbeen.eu", "ClusterAccess");
}

#[test]
fn all_crds_have_v1alpha1_version() {
    let crds = vec![
        AgentJob::crd(),
        AgentRuntime::crd(),
        AgentAuth::crd(),
        AgentRole::crd(),
        AgentSkill::crd(),
        McpServer::crd(),
        Repository::crd(),
        ClusterAccess::crd(),
    ];
    for crd in &crds {
        let versions: Vec<&str> = crd.spec.versions.iter().map(|v| v.name.as_str()).collect();
        assert!(
            versions.contains(&"v1alpha1"),
            "CRD {} must have v1alpha1 version, got {:?}",
            crd.metadata.name.as_deref().unwrap(),
            versions
        );
    }
}
