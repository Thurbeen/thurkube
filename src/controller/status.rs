//! Status subresource patching for AgentJob.

use chrono::Utc;
use kube::api::{Patch, PatchParams};
use kube::Api;

use crate::controller::{Error, FIELD_MANAGER};
use crate::crd::{AgentJob, AgentJobStatus, Condition, OwnedResource};

pub struct StatusUpdate {
    pub phase: &'static str,
    pub message: Option<String>,
    pub config_hash: Option<String>,
    pub owned: Vec<OwnedResource>,
    pub ready: Option<(bool, &'static str, String)>,
    pub last_run_time: Option<String>,
}

impl StatusUpdate {
    pub fn pending() -> Self {
        Self {
            phase: "Pending",
            message: None,
            config_hash: None,
            owned: vec![],
            ready: None,
            last_run_time: None,
        }
    }
}

pub async fn patch_status(
    api: &Api<AgentJob>,
    ajob: &AgentJob,
    update: StatusUpdate,
) -> Result<(), Error> {
    let name = ajob
        .metadata
        .name
        .as_deref()
        .ok_or_else(|| Error::InvalidSpec("AgentJob missing name".into()))?;

    let mut conditions = ajob
        .status
        .as_ref()
        .map(|s| s.conditions.clone())
        .unwrap_or_default();

    if let Some((ready, reason, msg)) = update.ready {
        set_condition(
            &mut conditions,
            Condition {
                r#type: "Ready".into(),
                status: if ready { "True".into() } else { "False".into() },
                last_transition_time: Utc::now().to_rfc3339(),
                reason: Some(reason.to_string()),
                message: Some(msg),
            },
        );
    }

    let status = AgentJobStatus {
        phase: Some(update.phase.to_string()),
        last_run_time: update
            .last_run_time
            .or_else(|| ajob.status.as_ref().and_then(|s| s.last_run_time.clone())),
        last_completion_time: ajob
            .status
            .as_ref()
            .and_then(|s| s.last_completion_time.clone()),
        message: update.message,
        observed_generation: ajob.metadata.generation.unwrap_or(0),
        config_hash: update.config_hash,
        owned_resources: update.owned,
        conditions,
    };

    if status_equivalent(ajob.status.as_ref(), &status) {
        return Ok(());
    }

    let patch = serde_json::json!({
        "apiVersion": "thurkube.thurbeen.eu/v1alpha1",
        "kind": "AgentJob",
        "status": status,
    });

    api.patch_status(
        name,
        &PatchParams::apply(FIELD_MANAGER).force(),
        &Patch::Apply(patch),
    )
    .await?;
    Ok(())
}

/// Compare an existing status to a newly-built one, ignoring
/// `lastTransitionTime` on conditions whose `status` has not
/// changed. Prevents feedback loops where every reconcile writes
/// status → triggers a watch event → reconciles again.
fn status_equivalent(old: Option<&AgentJobStatus>, new: &AgentJobStatus) -> bool {
    let Some(old) = old else {
        return false;
    };
    if old.phase != new.phase
        || old.message != new.message
        || old.observed_generation != new.observed_generation
        || old.config_hash != new.config_hash
        || old.last_run_time != new.last_run_time
        || old.last_completion_time != new.last_completion_time
        || old.owned_resources.len() != new.owned_resources.len()
    {
        return false;
    }
    for (a, b) in old.owned_resources.iter().zip(new.owned_resources.iter()) {
        if a.kind != b.kind || a.name != b.name {
            return false;
        }
    }
    if old.conditions.len() != new.conditions.len() {
        return false;
    }
    for (a, b) in old.conditions.iter().zip(new.conditions.iter()) {
        if a.r#type != b.r#type
            || a.status != b.status
            || a.reason != b.reason
            || a.message != b.message
        {
            return false;
        }
    }
    true
}

fn set_condition(conds: &mut Vec<Condition>, new: Condition) {
    if let Some(existing) = conds.iter_mut().find(|c| c.r#type == new.r#type) {
        if existing.status != new.status {
            *existing = new;
        } else {
            existing.reason = new.reason;
            existing.message = new.message;
        }
        return;
    }
    conds.push(new);
}
