//! Fetches CRD references from the cluster.

use kube::Api;
use kube::Client;

use crate::controller::Error;
use crate::crd::{
    AgentAuth, AgentJob, AgentRole, AgentRuntime, AgentSkill, ClusterAccess, McpServer, Repository,
};

pub struct Resolved {
    pub runtime: AgentRuntime,
    pub auth: AgentAuth,
    pub role: AgentRole,
    pub skill: Option<AgentSkill>,
    pub cluster_access: Option<ClusterAccess>,
    pub mcp_servers: Vec<(String, McpServer)>,
    pub repositories: Vec<(String, Repository)>,
}

pub async fn resolve(client: &Client, ajob: &AgentJob) -> Result<Resolved, Error> {
    let ns = ajob
        .metadata
        .namespace
        .as_deref()
        .ok_or_else(|| Error::InvalidSpec("AgentJob missing namespace".into()))?;

    let runtime =
        get_named::<AgentRuntime>(client, ns, &ajob.spec.runtime_ref, "AgentRuntime").await?;
    let auth = get_named::<AgentAuth>(client, ns, &ajob.spec.auth_ref, "AgentAuth").await?;
    let role = get_named::<AgentRole>(client, ns, &ajob.spec.role_ref, "AgentRole").await?;

    let skill = if let Some(name) = &ajob.spec.skill_ref {
        Some(get_named::<AgentSkill>(client, ns, name, "AgentSkill").await?)
    } else {
        None
    };

    let cluster_access = if let Some(name) = &ajob.spec.cluster_access_ref {
        Some(get_named::<ClusterAccess>(client, ns, name, "ClusterAccess").await?)
    } else {
        None
    };

    let mut mcp_servers = Vec::with_capacity(ajob.spec.mcp_server_refs.len());
    for name in &ajob.spec.mcp_server_refs {
        let s = get_named::<McpServer>(client, ns, name, "McpServer").await?;
        mcp_servers.push((name.clone(), s));
    }

    let mut repositories = Vec::with_capacity(ajob.spec.repository_refs.len());
    for name in &ajob.spec.repository_refs {
        let r = get_named::<Repository>(client, ns, name, "Repository").await?;
        repositories.push((name.clone(), r));
    }

    Ok(Resolved {
        runtime,
        auth,
        role,
        skill,
        cluster_access,
        mcp_servers,
        repositories,
    })
}

async fn get_named<K>(
    client: &Client,
    namespace: &str,
    name: &str,
    kind: &'static str,
) -> Result<K, Error>
where
    K: kube::Resource<Scope = k8s_openapi::NamespaceResourceScope>
        + Clone
        + std::fmt::Debug
        + for<'de> serde::Deserialize<'de>,
    <K as kube::Resource>::DynamicType: Default,
{
    let api: Api<K> = Api::namespaced(client.clone(), namespace);
    match api.get_opt(name).await? {
        Some(obj) => Ok(obj),
        None => Err(Error::RefNotFound {
            kind,
            name: name.to_owned(),
            namespace: namespace.to_owned(),
        }),
    }
}
