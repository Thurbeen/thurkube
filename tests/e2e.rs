//! End-to-end tests against a real Kubernetes cluster.
//!
//! These tests require a running cluster (k3d/kind in CI, any
//! kubeconfig locally). They are marked `#[ignore]` so they
//! don't run in the default `cargo nextest` pass.
//!
//! Run with:
//!   cargo test --test e2e -- --ignored --nocapture

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{Api, DeleteParams, ListParams, PostParams};
use kube::CustomResourceExt;
use serde_json::json;

use thurkube::crd::{
    AgentAuth, AgentJob, AgentRole, AgentRuntime, AgentSkill, ClusterAccess, McpServer, Repository,
};

// ── Helpers ─────────────────────────────────────────────────

const TEST_NS: &str = "default";

/// Verify a CRD is installed and established on the cluster.
async fn assert_crd_established(
    api: &Api<CustomResourceDefinition>,
    crd: &CustomResourceDefinition,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = crd.metadata.name.as_deref().unwrap();

    for _ in 0..30 {
        if let Ok(live) = api.get(name).await {
            let conditions = live
                .status
                .as_ref()
                .and_then(|s| s.conditions.as_ref())
                .cloned()
                .unwrap_or_default();
            if conditions
                .iter()
                .any(|c| c.type_ == "Established" && c.status == "True")
            {
                return Ok(());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    Err(format!("CRD {name} not established after 15s").into())
}

/// Delete a resource by name, ignoring NotFound errors.
async fn cleanup<T>(api: &Api<T>, name: &str)
where
    T: kube::Resource + Clone + serde::de::DeserializeOwned + std::fmt::Debug + serde::Serialize,
    <T as kube::Resource>::DynamicType: Default,
{
    let _ = api.delete(name, &DeleteParams::default()).await;
}

// ── CRD installation ────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn install_all_crds() {
    let client = kube::Client::try_default()
        .await
        .expect("kubeconfig must be available");

    let crd_api = Api::<CustomResourceDefinition>::all(client);

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
        let name = crd.metadata.name.as_deref().unwrap();
        assert_crd_established(&crd_api, crd)
            .await
            .unwrap_or_else(|e| panic!("CRD not ready {name}: {e}"));
        println!("CRD established: {name}");
    }
}

// ── AgentRuntime ────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn agent_runtime_crud() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<AgentRuntime>::namespaced(client, TEST_NS);

    cleanup(&api, "e2e-rt-crud").await;

    let cr = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "AgentRuntime",
        "metadata": { "name": "e2e-rt-crud" },
        "spec": {
            "image": "ghcr.io/thurbeen/claude-code-job:latest",
            "command": ["/usr/local/bin/entrypoint.sh"],
            "authEnvVar": "CLAUDE_CODE_OAUTH_TOKEN",
            "configPath": "/etc/claude-code-job",
            "persistPath": "/var/lib/claude-code-job"
        }
    }))
    .unwrap();

    // Create
    let created = api.create(&PostParams::default(), &cr).await.unwrap();
    assert_eq!(
        created.spec.image,
        "ghcr.io/thurbeen/claude-code-job:latest"
    );
    assert_eq!(created.spec.auth_env_var, "CLAUDE_CODE_OAUTH_TOKEN");
    assert_eq!(created.spec.config_path, "/etc/claude-code-job");
    assert_eq!(created.spec.persist_path, "/var/lib/claude-code-job");
    assert_eq!(created.spec.command, vec!["/usr/local/bin/entrypoint.sh"]);

    // Get
    let fetched = api.get("e2e-rt-crud").await.unwrap();
    assert_eq!(fetched.spec.image, created.spec.image);

    // List
    let list = api.list(&ListParams::default()).await.unwrap();
    assert!(list.items.iter().any(|r| r.spec.image.contains("claude")));

    // Delete
    api.delete("e2e-rt-crud", &DeleteParams::default())
        .await
        .unwrap();
    assert!(api.get("e2e-rt-crud").await.is_err());
}

#[tokio::test]
#[ignore]
async fn agent_runtime_without_command() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<AgentRuntime>::namespaced(client, TEST_NS);

    cleanup(&api, "e2e-rt-nocmd").await;

    let cr = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "AgentRuntime",
        "metadata": { "name": "e2e-rt-nocmd" },
        "spec": {
            "image": "ghcr.io/example/other-agent:v1",
            "authEnvVar": "API_KEY",
            "configPath": "/config",
            "persistPath": "/data"
        }
    }))
    .unwrap();

    let created = api.create(&PostParams::default(), &cr).await.unwrap();
    assert!(created.spec.command.is_empty());

    cleanup(&api, "e2e-rt-nocmd").await;
}

// ── AgentAuth ───────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn agent_auth_crud() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<AgentAuth>::namespaced(client, TEST_NS);

    cleanup(&api, "e2e-auth-crud").await;

    let cr = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "AgentAuth",
        "metadata": { "name": "e2e-auth-crud" },
        "spec": {
            "secretRef": {
                "name": "claude-code-jobs",
                "key": "CLAUDE_CODE_OAUTH_TOKEN"
            }
        }
    }))
    .unwrap();

    let created = api.create(&PostParams::default(), &cr).await.unwrap();
    assert_eq!(created.spec.secret_ref.name, "claude-code-jobs");
    assert_eq!(created.spec.secret_ref.key, "CLAUDE_CODE_OAUTH_TOKEN");

    cleanup(&api, "e2e-auth-crud").await;
}

// ── AgentRole ───────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn agent_role_with_mcp_wildcards() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<AgentRole>::namespaced(client, TEST_NS);

    cleanup(&api, "e2e-role-mcp").await;

    let cr = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "AgentRole",
        "metadata": { "name": "e2e-role-mcp" },
        "spec": {
            "allowedTools": [
                "Bash", "Read", "Glob", "Grep", "Edit", "Write",
                "mcp__gmail__*",
                "mcp__google-sheets__*"
            ]
        }
    }))
    .unwrap();

    let created = api.create(&PostParams::default(), &cr).await.unwrap();
    assert_eq!(created.spec.allowed_tools.len(), 8);
    assert!(created
        .spec
        .allowed_tools
        .contains(&"mcp__gmail__*".to_owned()));

    cleanup(&api, "e2e-role-mcp").await;
}

// ── AgentSkill ──────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn agent_skill_default_ref() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<AgentSkill>::namespaced(client, TEST_NS);

    cleanup(&api, "e2e-skill-def").await;

    // Omit `ref` — should default to "main" on the server
    let cr = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "AgentSkill",
        "metadata": { "name": "e2e-skill-def" },
        "spec": {
            "repo": "Thurbeen/thurbeen-skills",
            "name": "bump-pr-fixer"
        }
    }))
    .unwrap();

    let created = api.create(&PostParams::default(), &cr).await.unwrap();
    assert_eq!(created.spec.repo, "Thurbeen/thurbeen-skills");
    assert_eq!(created.spec.name, "bump-pr-fixer");
    assert_eq!(created.spec.r#ref, "main");

    cleanup(&api, "e2e-skill-def").await;
}

#[tokio::test]
#[ignore]
async fn agent_skill_explicit_ref() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<AgentSkill>::namespaced(client, TEST_NS);

    cleanup(&api, "e2e-skill-tag").await;

    let cr = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "AgentSkill",
        "metadata": { "name": "e2e-skill-tag" },
        "spec": {
            "repo": "Thurbeen/thurbeen-skills",
            "name": "infra-monitor",
            "ref": "v2.0.0"
        }
    }))
    .unwrap();

    let created = api.create(&PostParams::default(), &cr).await.unwrap();
    assert_eq!(created.spec.r#ref, "v2.0.0");

    cleanup(&api, "e2e-skill-tag").await;
}

// ── McpServer ───────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn mcp_server_command_based() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<McpServer>::namespaced(client, TEST_NS);

    cleanup(&api, "e2e-mcp-cmd").await;

    let cr = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "McpServer",
        "metadata": { "name": "e2e-mcp-cmd" },
        "spec": { "command": "npx", "args": ["-y", "gmail-mcp"] }
    }))
    .unwrap();

    let created = api.create(&PostParams::default(), &cr).await.unwrap();
    assert_eq!(created.spec.command.unwrap(), "npx");
    assert_eq!(created.spec.args, vec!["-y", "gmail-mcp"]);
    assert!(created.spec.url.is_none());

    cleanup(&api, "e2e-mcp-cmd").await;
}

#[tokio::test]
#[ignore]
async fn mcp_server_url_based() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<McpServer>::namespaced(client, TEST_NS);

    cleanup(&api, "e2e-mcp-url").await;

    let cr = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "McpServer",
        "metadata": { "name": "e2e-mcp-url" },
        "spec": { "url": "https://dev.helloasso.com/mcp" }
    }))
    .unwrap();

    let created = api.create(&PostParams::default(), &cr).await.unwrap();
    assert!(created.spec.command.is_none());
    assert!(created.spec.args.is_empty());
    assert_eq!(created.spec.url.unwrap(), "https://dev.helloasso.com/mcp");

    cleanup(&api, "e2e-mcp-url").await;
}

// ── Repository ──────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn repository_with_token() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<Repository>::namespaced(client, TEST_NS);

    cleanup(&api, "e2e-repo-tok").await;

    let cr = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "Repository",
        "metadata": { "name": "e2e-repo-tok" },
        "spec": {
            "owner": "Thurbeen",
            "name": "thurspace",
            "tokenSecretRef": { "name": "gh-tokens", "key": "GH_TOKEN" }
        }
    }))
    .unwrap();

    let created = api.create(&PostParams::default(), &cr).await.unwrap();
    assert_eq!(created.spec.owner, "Thurbeen");
    assert_eq!(created.spec.name, "thurspace");
    let token_ref = created.spec.token_secret_ref.unwrap();
    assert_eq!(token_ref.name, "gh-tokens");
    assert_eq!(token_ref.key, "GH_TOKEN");

    cleanup(&api, "e2e-repo-tok").await;
}

#[tokio::test]
#[ignore]
async fn repository_without_token() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<Repository>::namespaced(client, TEST_NS);

    cleanup(&api, "e2e-repo-pub").await;

    let cr = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "Repository",
        "metadata": { "name": "e2e-repo-pub" },
        "spec": {
            "owner": "Thurbeen",
            "name": "gdcruiser"
        }
    }))
    .unwrap();

    let created = api.create(&PostParams::default(), &cr).await.unwrap();
    assert!(created.spec.token_secret_ref.is_none());

    cleanup(&api, "e2e-repo-pub").await;
}

// ── ClusterAccess ───────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn cluster_access_multiple_rules() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<ClusterAccess>::namespaced(client, TEST_NS);

    cleanup(&api, "e2e-ca-multi").await;

    let cr = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "ClusterAccess",
        "metadata": { "name": "e2e-ca-multi" },
        "spec": {
            "rules": [
                {
                    "apiGroups": [""],
                    "resources": ["pods", "events", "nodes", "services"],
                    "verbs": ["get", "list"]
                },
                {
                    "apiGroups": ["apps"],
                    "resources": ["deployments", "statefulsets"],
                    "verbs": ["get", "list"]
                },
                {
                    "apiGroups": ["rbac.authorization.k8s.io"],
                    "resources": ["roles", "clusterroles"],
                    "verbs": ["get", "list"]
                }
            ]
        }
    }))
    .unwrap();

    let created = api.create(&PostParams::default(), &cr).await.unwrap();
    assert_eq!(created.spec.rules.len(), 3);
    assert_eq!(created.spec.rules[0].resources.as_ref().unwrap().len(), 4);
    assert_eq!(
        created.spec.rules[2].api_groups.as_ref().unwrap(),
        &vec!["rbac.authorization.k8s.io".to_owned()]
    );

    cleanup(&api, "e2e-ca-multi").await;
}

// ── AgentJob ────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn agent_job_minimal() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<AgentJob>::namespaced(client.clone(), TEST_NS);

    // Prereqs
    let rt_api = Api::<AgentRuntime>::namespaced(client.clone(), TEST_NS);
    let auth_api = Api::<AgentAuth>::namespaced(client.clone(), TEST_NS);
    let role_api = Api::<AgentRole>::namespaced(client.clone(), TEST_NS);

    cleanup(&api, "e2e-job-min").await;
    cleanup(&rt_api, "e2e-job-min-rt").await;
    cleanup(&auth_api, "e2e-job-min-auth").await;
    cleanup(&role_api, "e2e-job-min-role").await;

    rt_api
        .create(
            &PostParams::default(),
            &serde_json::from_value(json!({
                "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
                "kind": "AgentRuntime",
                "metadata": { "name": "e2e-job-min-rt" },
                "spec": {
                    "image": "agent:latest",
                    "authEnvVar": "TOKEN",
                    "configPath": "/cfg",
                    "persistPath": "/data"
                }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

    auth_api
        .create(
            &PostParams::default(),
            &serde_json::from_value(json!({
                "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
                "kind": "AgentAuth",
                "metadata": { "name": "e2e-job-min-auth" },
                "spec": { "secretRef": { "name": "s", "key": "k" } }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

    role_api
        .create(
            &PostParams::default(),
            &serde_json::from_value(json!({
                "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
                "kind": "AgentRole",
                "metadata": { "name": "e2e-job-min-role" },
                "spec": { "allowedTools": ["Bash"] }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

    // Minimal AgentJob — only required fields
    let job: AgentJob = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "AgentJob",
        "metadata": { "name": "e2e-job-min" },
        "spec": {
            "runtimeRef": "e2e-job-min-rt",
            "authRef": "e2e-job-min-auth",
            "roleRef": "e2e-job-min-role",
            "prompt": "Hello."
        }
    }))
    .unwrap();

    let created = api.create(&PostParams::default(), &job).await.unwrap();

    // Verify defaults
    assert_eq!(created.spec.timezone, "Etc/UTC");
    assert_eq!(created.spec.timeout_seconds, 3600);
    assert!(!created.spec.suspend);
    assert!(!created.spec.persist);
    assert!(created.spec.schedule.is_none());
    assert!(created.spec.skill_ref.is_none());
    assert!(created.spec.cluster_access_ref.is_none());
    assert!(created.spec.instructions.is_none());
    assert!(created.spec.model.is_none());
    assert!(created.spec.resources.is_none());
    assert!(created.spec.mcp_server_refs.is_empty());
    assert!(created.spec.repository_refs.is_empty());
    assert!(created.spec.env.is_empty());
    assert!(created.spec.env_from.is_empty());

    cleanup(&api, "e2e-job-min").await;
    cleanup(&role_api, "e2e-job-min-role").await;
    cleanup(&auth_api, "e2e-job-min-auth").await;
    cleanup(&rt_api, "e2e-job-min-rt").await;
}

#[tokio::test]
#[ignore]
async fn agent_job_full() {
    let client = kube::Client::try_default().await.unwrap();
    let api = Api::<AgentJob>::namespaced(client.clone(), TEST_NS);

    // Prereqs
    let rt_api = Api::<AgentRuntime>::namespaced(client.clone(), TEST_NS);
    let auth_api = Api::<AgentAuth>::namespaced(client.clone(), TEST_NS);
    let role_api = Api::<AgentRole>::namespaced(client.clone(), TEST_NS);
    let skill_api = Api::<AgentSkill>::namespaced(client.clone(), TEST_NS);
    let mcp_api = Api::<McpServer>::namespaced(client.clone(), TEST_NS);
    let repo_api = Api::<Repository>::namespaced(client.clone(), TEST_NS);
    let ca_api = Api::<ClusterAccess>::namespaced(client.clone(), TEST_NS);

    // Cleanup
    for name in ["e2e-job-full"] {
        cleanup(&api, name).await;
    }
    cleanup(&rt_api, "e2e-jf-rt").await;
    cleanup(&auth_api, "e2e-jf-auth").await;
    cleanup(&role_api, "e2e-jf-role").await;
    cleanup(&skill_api, "e2e-jf-skill").await;
    cleanup(&mcp_api, "e2e-jf-mcp").await;
    cleanup(&repo_api, "e2e-jf-repo").await;
    cleanup(&ca_api, "e2e-jf-ca").await;

    // Create all referenced resources
    rt_api
        .create(
            &PostParams::default(),
            &serde_json::from_value(json!({
                "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
                "kind": "AgentRuntime",
                "metadata": { "name": "e2e-jf-rt" },
                "spec": {
                    "image": "agent:v1",
                    "authEnvVar": "TOK",
                    "configPath": "/c",
                    "persistPath": "/d"
                }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

    auth_api
        .create(
            &PostParams::default(),
            &serde_json::from_value(json!({
                "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
                "kind": "AgentAuth",
                "metadata": { "name": "e2e-jf-auth" },
                "spec": { "secretRef": { "name": "s", "key": "k" } }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

    role_api
        .create(
            &PostParams::default(),
            &serde_json::from_value(json!({
                "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
                "kind": "AgentRole",
                "metadata": { "name": "e2e-jf-role" },
                "spec": { "allowedTools": ["Bash", "Read"] }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

    skill_api
        .create(
            &PostParams::default(),
            &serde_json::from_value(json!({
                "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
                "kind": "AgentSkill",
                "metadata": { "name": "e2e-jf-skill" },
                "spec": { "repo": "Thurbeen/thurbeen-skills", "name": "monitor" }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

    mcp_api
        .create(
            &PostParams::default(),
            &serde_json::from_value(json!({
                "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
                "kind": "McpServer",
                "metadata": { "name": "e2e-jf-mcp" },
                "spec": { "command": "npx", "args": ["-y", "test-mcp"] }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

    repo_api
        .create(
            &PostParams::default(),
            &serde_json::from_value(json!({
                "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
                "kind": "Repository",
                "metadata": { "name": "e2e-jf-repo" },
                "spec": { "owner": "Thurbeen", "name": "thurspace" }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

    ca_api
        .create(
            &PostParams::default(),
            &serde_json::from_value(json!({
                "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
                "kind": "ClusterAccess",
                "metadata": { "name": "e2e-jf-ca" },
                "spec": { "rules": [{ "apiGroups": [""], "resources": ["pods"], "verbs": ["get"] }] }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

    // Full AgentJob — all fields populated
    let job: AgentJob = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "AgentJob",
        "metadata": { "name": "e2e-job-full" },
        "spec": {
            "schedule": "0 */6 * * *",
            "timezone": "Europe/Paris",
            "suspend": true,
            "runtimeRef": "e2e-jf-rt",
            "authRef": "e2e-jf-auth",
            "roleRef": "e2e-jf-role",
            "skillRef": "e2e-jf-skill",
            "clusterAccessRef": "e2e-jf-ca",
            "mcpServerRefs": ["e2e-jf-mcp"],
            "repositoryRefs": ["e2e-jf-repo"],
            "prompt": "Run the full integration test.",
            "instructions": "Extra context for the agent.",
            "model": "sonnet",
            "timeoutSeconds": 7200,
            "persist": true,
            "env": [
                { "name": "REPOS", "value": "Thurbeen/thurspace" }
            ],
            "envFrom": [
                { "secretRef": { "name": "extra-secrets" } }
            ]
        }
    }))
    .unwrap();

    let created = api.create(&PostParams::default(), &job).await.unwrap();

    assert_eq!(created.spec.schedule.unwrap(), "0 */6 * * *");
    assert_eq!(created.spec.timezone, "Europe/Paris");
    assert!(created.spec.suspend);
    assert_eq!(created.spec.runtime_ref, "e2e-jf-rt");
    assert_eq!(created.spec.auth_ref, "e2e-jf-auth");
    assert_eq!(created.spec.role_ref, "e2e-jf-role");
    assert_eq!(created.spec.skill_ref.unwrap(), "e2e-jf-skill");
    assert_eq!(created.spec.cluster_access_ref.unwrap(), "e2e-jf-ca");
    assert_eq!(created.spec.mcp_server_refs, vec!["e2e-jf-mcp"]);
    assert_eq!(created.spec.repository_refs, vec!["e2e-jf-repo"]);
    assert_eq!(created.spec.prompt, "Run the full integration test.");
    assert_eq!(
        created.spec.instructions.unwrap(),
        "Extra context for the agent."
    );
    assert_eq!(created.spec.model.unwrap(), "sonnet");
    assert_eq!(created.spec.timeout_seconds, 7200);
    assert!(created.spec.persist);
    assert_eq!(created.spec.env.len(), 1);
    assert_eq!(created.spec.env[0].name, "REPOS");
    assert_eq!(created.spec.env_from.len(), 1);

    // Cleanup
    cleanup(&api, "e2e-job-full").await;
    cleanup(&ca_api, "e2e-jf-ca").await;
    cleanup(&repo_api, "e2e-jf-repo").await;
    cleanup(&mcp_api, "e2e-jf-mcp").await;
    cleanup(&skill_api, "e2e-jf-skill").await;
    cleanup(&role_api, "e2e-jf-role").await;
    cleanup(&auth_api, "e2e-jf-auth").await;
    cleanup(&rt_api, "e2e-jf-rt").await;
}

// ── Validation / rejection tests ────────────────────────────

fn dynamic_api(client: kube::Client, kind: &str, plural: &str) -> Api<kube::api::DynamicObject> {
    let ar = kube::api::ApiResource {
        group: "thurkube.thurbeen.eu".into(),
        version: "v1alpha1".into(),
        api_version: "thurkube.thurbeen.eu/v1alpha1".into(),
        kind: kind.into(),
        plural: plural.into(),
    };
    Api::namespaced_with(client, TEST_NS, &ar)
}

#[tokio::test]
#[ignore]
async fn reject_agent_job_missing_required_fields() {
    let client = kube::Client::try_default().await.unwrap();
    let api = dynamic_api(client, "AgentJob", "agentjobs");

    // Missing prompt, runtimeRef, authRef, roleRef
    let invalid: kube::api::DynamicObject = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "AgentJob",
        "metadata": { "name": "e2e-invalid-job" },
        "spec": {
            "schedule": "0 * * * *"
        }
    }))
    .unwrap();

    let result = api.create(&PostParams::default(), &invalid).await;
    assert!(
        result.is_err(),
        "Creating AgentJob without required fields should fail"
    );
}

#[tokio::test]
#[ignore]
async fn reject_agent_runtime_missing_image() {
    let client = kube::Client::try_default().await.unwrap();
    let api = dynamic_api(client, "AgentRuntime", "agentruntimes");

    let invalid: kube::api::DynamicObject = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "AgentRuntime",
        "metadata": { "name": "e2e-invalid-rt" },
        "spec": {
            "authEnvVar": "TOKEN"
        }
    }))
    .unwrap();

    let result = api.create(&PostParams::default(), &invalid).await;
    assert!(
        result.is_err(),
        "Creating AgentRuntime without image should fail"
    );
}

#[tokio::test]
#[ignore]
async fn reject_cluster_access_empty_rules() {
    let client = kube::Client::try_default().await.unwrap();
    let api = dynamic_api(client, "ClusterAccess", "clusteraccesses");

    // Missing rules entirely
    let invalid: kube::api::DynamicObject = serde_json::from_value(json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "ClusterAccess",
        "metadata": { "name": "e2e-invalid-ca" },
        "spec": {}
    }))
    .unwrap();

    let result = api.create(&PostParams::default(), &invalid).await;
    assert!(
        result.is_err(),
        "Creating ClusterAccess without rules should fail"
    );
}
