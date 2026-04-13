//! Pure builders for child resources owned by an AgentJob.

use std::collections::BTreeMap;

use k8s_openapi::api::batch::v1::{CronJob, CronJobSpec, Job, JobSpec, JobTemplateSpec};
use k8s_openapi::api::core::v1::{
    ConfigMap, ConfigMapVolumeSource, Container, EmptyDirVolumeSource, EnvVar, EnvVarSource,
    PersistentVolumeClaim, PersistentVolumeClaimSpec, PersistentVolumeClaimVolumeSource, PodSpec,
    PodTemplateSpec, ResourceRequirements, SecretKeySelector, SecurityContext, ServiceAccount,
    Volume, VolumeMount, VolumeResourceRequirements,
};
use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding, RoleRef, Subject};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
use kube::ResourceExt;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::controller::resolve::Resolved;
use crate::controller::{LABEL_AGENTJOB, LABEL_AGENTJOB_NS, LABEL_MANAGED_BY, LABEL_OWNER_UID};
use crate::crd::AgentJob;

const POD_USER: i64 = 65532;
const AGENT_HOME: &str = "/home/agent";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig<'a> {
    pub prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<&'a str>,
    pub allowed_tools: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<SkillRef<'a>>,
    pub mcp_servers: Vec<McpServerRef<'a>>,
    pub repositories: Vec<RepoRef<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRef<'a> {
    pub repo: &'a str,
    pub name: &'a str,
    pub r#ref: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerRef<'a> {
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRef<'a> {
    pub name: &'a str,
    pub owner: &'a str,
    pub repo: &'a str,
}

pub fn agent_config_json(ajob: &AgentJob, r: &Resolved) -> serde_json::Result<String> {
    let cfg = AgentConfig {
        prompt: &ajob.spec.prompt,
        instructions: ajob.spec.instructions.as_deref(),
        model: ajob.spec.model.as_deref(),
        allowed_tools: &r.role.spec.allowed_tools,
        skill: r.skill.as_ref().map(|s| SkillRef {
            repo: &s.spec.repo,
            name: &s.spec.name,
            r#ref: &s.spec.r#ref,
        }),
        mcp_servers: r
            .mcp_servers
            .iter()
            .map(|(name, m)| McpServerRef {
                name,
                command: m.spec.command.as_deref(),
                args: m.spec.args.iter().map(String::as_str).collect(),
                url: m.spec.url.as_deref(),
            })
            .collect(),
        repositories: r
            .repositories
            .iter()
            .map(|(name, repo)| RepoRef {
                name,
                owner: &repo.spec.owner,
                repo: &repo.spec.name,
            })
            .collect(),
    };
    serde_json::to_string_pretty(&cfg)
}

pub fn config_hash(data: &str) -> String {
    let mut h = Sha256::new();
    h.update(data.as_bytes());
    let bytes = h.finalize();
    hex_short(&bytes)
}

fn hex_short(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(16);
    for b in &bytes[..8] {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

pub fn labels(ajob: &AgentJob) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert(LABEL_MANAGED_BY.into(), "thurkube".into());
    m.insert(LABEL_AGENTJOB.into(), ajob.name_any());
    if let Some(ns) = &ajob.metadata.namespace {
        m.insert(LABEL_AGENTJOB_NS.into(), ns.clone());
    }
    if let Some(uid) = ajob.metadata.uid.as_deref() {
        m.insert(LABEL_OWNER_UID.into(), uid.to_owned());
    }
    m
}

fn owner_ref(ajob: &AgentJob) -> Option<OwnerReference> {
    let name = ajob.metadata.name.clone()?;
    let uid = ajob.metadata.uid.clone()?;
    Some(OwnerReference {
        api_version: "thurkube.thurbeen.eu/v1alpha1".into(),
        kind: "AgentJob".into(),
        name,
        uid,
        controller: Some(true),
        block_owner_deletion: Some(true),
    })
}

fn namespaced_meta(name: String, namespace: &str, ajob: &AgentJob) -> ObjectMeta {
    ObjectMeta {
        name: Some(name),
        namespace: Some(namespace.to_owned()),
        labels: Some(labels(ajob)),
        owner_references: owner_ref(ajob).map(|o| vec![o]),
        ..Default::default()
    }
}

fn cluster_meta(name: String, ajob: &AgentJob) -> ObjectMeta {
    ObjectMeta {
        name: Some(name),
        labels: Some(labels(ajob)),
        ..Default::default()
    }
}

pub fn configmap(ajob: &AgentJob, namespace: &str, agent_json: &str) -> ConfigMap {
    let mut data = BTreeMap::new();
    data.insert("agent.json".into(), agent_json.to_owned());
    ConfigMap {
        metadata: namespaced_meta(config_name(ajob), namespace, ajob),
        data: Some(data),
        ..Default::default()
    }
}

pub fn pvc(ajob: &AgentJob, namespace: &str) -> PersistentVolumeClaim {
    let mut requests = BTreeMap::new();
    requests.insert("storage".into(), Quantity("1Gi".into()));
    PersistentVolumeClaim {
        metadata: namespaced_meta(pvc_name(ajob), namespace, ajob),
        spec: Some(PersistentVolumeClaimSpec {
            access_modes: Some(vec!["ReadWriteOnce".into()]),
            resources: Some(VolumeResourceRequirements {
                requests: Some(requests),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn service_account(ajob: &AgentJob, namespace: &str) -> ServiceAccount {
    ServiceAccount {
        metadata: namespaced_meta(runner_name(ajob), namespace, ajob),
        automount_service_account_token: Some(true),
        ..Default::default()
    }
}

pub fn cluster_role(ajob: &AgentJob, r: &Resolved) -> ClusterRole {
    ClusterRole {
        metadata: cluster_meta(cluster_rbac_name(ajob), ajob),
        rules: r.cluster_access.as_ref().map(|ca| ca.spec.rules.clone()),
        ..Default::default()
    }
}

pub fn cluster_role_binding(ajob: &AgentJob, namespace: &str) -> ClusterRoleBinding {
    let name = cluster_rbac_name(ajob);
    ClusterRoleBinding {
        metadata: cluster_meta(name.clone(), ajob),
        role_ref: RoleRef {
            api_group: "rbac.authorization.k8s.io".into(),
            kind: "ClusterRole".into(),
            name,
        },
        subjects: Some(vec![Subject {
            kind: "ServiceAccount".into(),
            name: runner_name(ajob),
            namespace: Some(namespace.to_owned()),
            ..Default::default()
        }]),
    }
}

fn pod_template(ajob: &AgentJob, r: &Resolved, namespace: &str) -> PodTemplateSpec {
    let mut env = ajob.spec.env.clone();
    env.push(EnvVar {
        name: r.runtime.spec.auth_env_var.clone(),
        value_from: Some(EnvVarSource {
            secret_key_ref: Some(SecretKeySelector {
                name: r.auth.spec.secret_ref.name.clone(),
                key: r.auth.spec.secret_ref.key.clone(),
                optional: Some(false),
            }),
            ..Default::default()
        }),
        ..Default::default()
    });

    let mut volume_mounts = vec![VolumeMount {
        name: "agent-config".into(),
        mount_path: r.runtime.spec.config_path.clone(),
        read_only: Some(true),
        ..Default::default()
    }];
    let mut volumes = vec![Volume {
        name: "agent-config".into(),
        config_map: Some(ConfigMapVolumeSource {
            name: config_name(ajob),
            ..Default::default()
        }),
        ..Default::default()
    }];

    if ajob.spec.persist {
        volume_mounts.push(VolumeMount {
            name: "agent-persist".into(),
            mount_path: r.runtime.spec.persist_path.clone(),
            ..Default::default()
        });
        volumes.push(Volume {
            name: "agent-persist".into(),
            persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                claim_name: pvc_name(ajob),
                read_only: Some(false),
            }),
            ..Default::default()
        });
    }

    // Writable scratch dirs needed because the container runs
    // with readOnlyRootFilesystem=true. Backing them with
    // emptyDir keeps every AgentJob image working out-of-the-box
    // without per-image config.
    for (name, path) in [("tmp", "/tmp"), ("home", AGENT_HOME)] {
        volume_mounts.push(VolumeMount {
            name: name.into(),
            mount_path: path.into(),
            ..Default::default()
        });
        volumes.push(Volume {
            name: name.into(),
            empty_dir: Some(EmptyDirVolumeSource::default()),
            ..Default::default()
        });
    }

    // Force HOME to the writable emptyDir so tools that respect
    // $HOME (claude, gh, npm, git, ssh) write to the right place.
    env.push(EnvVar {
        name: "HOME".into(),
        value: Some(AGENT_HOME.into()),
        ..Default::default()
    });

    let command = if r.runtime.spec.command.is_empty() {
        None
    } else {
        Some(r.runtime.spec.command.clone())
    };

    let container = Container {
        name: "agent".into(),
        image: Some(r.runtime.spec.image.clone()),
        command,
        env: Some(env),
        env_from: if ajob.spec.env_from.is_empty() {
            None
        } else {
            Some(ajob.spec.env_from.clone())
        },
        volume_mounts: Some(volume_mounts),
        resources: ajob
            .spec
            .resources
            .clone()
            .or_else(|| Some(ResourceRequirements::default())),
        security_context: Some(SecurityContext {
            allow_privilege_escalation: Some(false),
            read_only_root_filesystem: Some(true),
            capabilities: Some(k8s_openapi::api::core::v1::Capabilities {
                drop: Some(vec!["ALL".into()]),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let sa_name = if ajob.spec.cluster_access_ref.is_some() {
        runner_name(ajob)
    } else {
        "default".into()
    };

    let pod_meta = ObjectMeta {
        labels: Some(labels(ajob)),
        namespace: Some(namespace.to_owned()),
        ..Default::default()
    };

    PodTemplateSpec {
        metadata: Some(pod_meta),
        spec: Some(PodSpec {
            containers: vec![container],
            restart_policy: Some("Never".into()),
            service_account_name: Some(sa_name),
            automount_service_account_token: Some(ajob.spec.cluster_access_ref.is_some()),
            security_context: Some(k8s_openapi::api::core::v1::PodSecurityContext {
                run_as_non_root: Some(true),
                run_as_user: Some(POD_USER),
                run_as_group: Some(POD_USER),
                fs_group: Some(POD_USER),
                seccomp_profile: Some(k8s_openapi::api::core::v1::SeccompProfile {
                    type_: "RuntimeDefault".into(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            volumes: Some(volumes),
            active_deadline_seconds: Some(i64::from(ajob.spec.timeout_seconds)),
            ..Default::default()
        }),
    }
}

pub fn job(ajob: &AgentJob, r: &Resolved, namespace: &str, hash: &str) -> Job {
    Job {
        metadata: namespaced_meta(job_name(ajob, hash), namespace, ajob),
        spec: Some(JobSpec {
            template: pod_template(ajob, r, namespace),
            backoff_limit: Some(0),
            active_deadline_seconds: Some(i64::from(ajob.spec.timeout_seconds)),
            ttl_seconds_after_finished: Some(86_400),
            selector: None,
            manual_selector: None,
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn cron_job(ajob: &AgentJob, r: &Resolved, namespace: &str, schedule: &str) -> CronJob {
    CronJob {
        metadata: namespaced_meta(cron_name(ajob), namespace, ajob),
        spec: Some(CronJobSpec {
            schedule: schedule.to_owned(),
            time_zone: Some(ajob.spec.timezone.clone()),
            suspend: Some(ajob.spec.suspend),
            concurrency_policy: Some("Forbid".into()),
            successful_jobs_history_limit: Some(3),
            failed_jobs_history_limit: Some(3),
            job_template: JobTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels(ajob)),
                    ..Default::default()
                }),
                spec: Some(JobSpec {
                    template: pod_template(ajob, r, namespace),
                    backoff_limit: Some(0),
                    active_deadline_seconds: Some(i64::from(ajob.spec.timeout_seconds)),
                    ttl_seconds_after_finished: Some(86_400),
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        status: None,
    }
}

pub fn config_name(ajob: &AgentJob) -> String {
    format!("{}-config", ajob.name_any())
}

pub fn runner_name(ajob: &AgentJob) -> String {
    format!("{}-runner", ajob.name_any())
}

pub fn pvc_name(ajob: &AgentJob) -> String {
    format!("{}-data", ajob.name_any())
}

pub fn cron_name(ajob: &AgentJob) -> String {
    ajob.name_any()
}

pub fn job_name(ajob: &AgentJob, hash: &str) -> String {
    format!("{}-{}", ajob.name_any(), hash)
}

pub fn cluster_rbac_name(ajob: &AgentJob) -> String {
    let ns = ajob.metadata.namespace.as_deref().unwrap_or("default");
    format!("thurkube-{ns}-{}", ajob.name_any())
}

pub fn owner_label_selector(ajob: &AgentJob) -> Option<String> {
    let uid = ajob.metadata.uid.as_deref()?;
    Some(format!("{LABEL_OWNER_UID}={uid}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crd::{
        AgentAuth, AgentAuthSpec, AgentJobSpec, AgentRole, AgentRoleSpec, AgentRuntime,
        AgentRuntimeSpec, SecretKeyRef,
    };
    use kube::api::ObjectMeta as KubeObjectMeta;

    fn mk_ajob() -> AgentJob {
        AgentJob {
            metadata: KubeObjectMeta {
                name: Some("hello".into()),
                namespace: Some("default".into()),
                uid: Some("uid-123".into()),
                ..Default::default()
            },
            spec: AgentJobSpec {
                schedule: None,
                timezone: "Etc/UTC".into(),
                suspend: false,
                runtime_ref: "rt".into(),
                auth_ref: "auth".into(),
                role_ref: "role".into(),
                skill_ref: None,
                cluster_access_ref: None,
                mcp_server_refs: vec![],
                repository_refs: vec![],
                prompt: "hi".into(),
                instructions: None,
                model: None,
                resources: None,
                timeout_seconds: 3600,
                env: vec![],
                env_from: vec![],
                persist: false,
            },
            status: None,
        }
    }

    fn mk_resolved() -> Resolved {
        Resolved {
            runtime: AgentRuntime {
                metadata: KubeObjectMeta::default(),
                spec: AgentRuntimeSpec {
                    image: "img:latest".into(),
                    command: vec![],
                    auth_env_var: "TOKEN".into(),
                    config_path: "/etc/agent".into(),
                    persist_path: "/var/agent".into(),
                },
            },
            auth: AgentAuth {
                metadata: KubeObjectMeta::default(),
                spec: AgentAuthSpec {
                    secret_ref: SecretKeyRef {
                        name: "tok".into(),
                        key: "TOKEN".into(),
                    },
                },
            },
            role: AgentRole {
                metadata: KubeObjectMeta::default(),
                spec: AgentRoleSpec {
                    allowed_tools: vec!["Bash".into()],
                },
            },
            skill: None,
            cluster_access: None,
            mcp_servers: vec![],
            repositories: vec![],
        }
    }

    #[test]
    fn build_job_basic() {
        let ajob = mk_ajob();
        let r = mk_resolved();
        let j = job(&ajob, &r, "default", "deadbeef");
        assert_eq!(j.metadata.name.as_deref(), Some("hello-deadbeef"));
        let spec = j.spec.unwrap();
        let container = &spec.template.spec.unwrap().containers[0];
        assert_eq!(container.image.as_deref(), Some("img:latest"));
        let env = container.env.as_ref().unwrap();
        assert!(env.iter().any(|e| e.name == "TOKEN"));
    }

    #[test]
    fn build_cronjob_has_schedule() {
        let ajob = mk_ajob();
        let r = mk_resolved();
        let cj = cron_job(&ajob, &r, "default", "0 */6 * * *");
        let spec = cj.spec.unwrap();
        assert_eq!(spec.schedule, "0 */6 * * *");
        assert_eq!(spec.time_zone.as_deref(), Some("Etc/UTC"));
        assert_eq!(spec.suspend, Some(false));
    }

    #[test]
    fn configmap_contains_agent_json() {
        let ajob = mk_ajob();
        let cm = configmap(&ajob, "default", r#"{"prompt":"hi"}"#);
        assert!(cm.data.unwrap().contains_key("agent.json"));
    }

    #[test]
    fn config_hash_is_stable() {
        assert_eq!(config_hash("x"), config_hash("x"));
        assert_ne!(config_hash("x"), config_hash("y"));
        assert_eq!(config_hash("x").len(), 16);
    }

    #[test]
    fn labels_include_owner_uid() {
        let ajob = mk_ajob();
        let l = labels(&ajob);
        assert_eq!(l.get(LABEL_OWNER_UID).map(String::as_str), Some("uid-123"));
        assert_eq!(l.get(LABEL_AGENTJOB).map(String::as_str), Some("hello"));
    }
}
