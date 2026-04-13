use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use kube::runtime::events::Recorder;
use kube::Client;

/// Shared state handed to every reconcile invocation.
pub struct Ctx {
    pub client: Client,
    pub recorder: Recorder,
    pub ready: Arc<AtomicBool>,
}
