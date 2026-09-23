use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use thiserror::Error;
use tokio::sync::watch;
use tokio::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    pub worker_threads: usize,
    pub max_blocking_threads: usize,
    pub shutdown_timeout: Duration,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            worker_threads: 2,
            max_blocking_threads: 4,
            shutdown_timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime configuration is outside its safe bounds")]
    InvalidConfiguration,
    #[error("runtime could not be initialized")]
    Initialization,
}

pub struct KernelRuntime {
    runtime: tokio::runtime::Runtime,
    shutdown_timeout: Duration,
}

impl KernelRuntime {
    pub fn new(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        if !(1..=64).contains(&config.worker_threads)
            || !(1..=256).contains(&config.max_blocking_threads)
            || config.shutdown_timeout.is_zero()
        {
            return Err(RuntimeError::InvalidConfiguration);
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config.worker_threads)
            .max_blocking_threads(config.max_blocking_threads)
            .enable_time()
            .build()
            .map_err(|_| RuntimeError::Initialization)?;
        Ok(Self {
            runtime,
            shutdown_timeout: config.shutdown_timeout,
        })
    }

    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    pub fn shutdown(self) {
        self.runtime.shutdown_timeout(self.shutdown_timeout);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationReason {
    UserRequest,
    ParentCancelled,
    DeadlineExceeded,
    ResourceRevoked,
    ModuleStop,
    KernelShutdown,
    PolicyRevoked,
    DependencyLost,
    Superseded,
    FailureCascade,
}

struct CancellationNode {
    sender: watch::Sender<Option<CancellationReason>>,
    parent: Option<Arc<CancellationNode>>,
}

#[derive(Clone)]
pub struct CancellationToken {
    node: Arc<CancellationNode>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::root()
    }

    pub fn root() -> Self {
        let (sender, _) = watch::channel(None);
        Self {
            node: Arc::new(CancellationNode {
                sender,
                parent: None,
            }),
        }
    }

    pub fn child_token(&self) -> Self {
        let (sender, _) = watch::channel(None);
        Self {
            node: Arc::new(CancellationNode {
                sender,
                parent: Some(Arc::clone(&self.node)),
            }),
        }
    }

    pub fn cancel(&self) {
        self.cancel_with(CancellationReason::UserRequest);
    }

    pub fn cancel_with(&self, reason: CancellationReason) {
        self.node.sender.send_replace(Some(reason));
    }

    pub fn is_cancelled(&self) -> bool {
        self.reason().is_some()
    }

    pub fn reason(&self) -> Option<CancellationReason> {
        (*self.node.sender.borrow()).or_else(|| {
            self.node.parent.as_ref().and_then(|parent| {
                let token = Self {
                    node: Arc::clone(parent),
                };
                token.reason()
            })
        })
    }

    pub async fn cancelled(&self) {
        let _ = wait_cancellation(Arc::clone(&self.node)).await;
    }

    pub fn cancelled_reason(
        &self,
    ) -> Pin<Box<dyn Future<Output = CancellationReason> + Send + '_>> {
        wait_cancellation(Arc::clone(&self.node))
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

fn wait_cancellation(
    node: Arc<CancellationNode>,
) -> Pin<Box<dyn Future<Output = CancellationReason> + Send>> {
    Box::pin(async move {
        let mut receiver = node.sender.subscribe();
        if let Some(reason) = *receiver.borrow_and_update() {
            return reason;
        }
        if let Some(parent) = node.parent.as_ref() {
            tokio::select! {
                reason = wait_signal(&mut receiver) => reason,
                reason = wait_cancellation(Arc::clone(parent)) => reason,
            }
        } else {
            wait_signal(&mut receiver).await
        }
    })
}

async fn wait_signal(
    receiver: &mut watch::Receiver<Option<CancellationReason>>,
) -> CancellationReason {
    loop {
        if let Some(reason) = *receiver.borrow_and_update() {
            return reason;
        }
        if receiver.changed().await.is_err() {
            return CancellationReason::KernelShutdown;
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Deadline {
    expires_at: Instant,
}

impl Deadline {
    pub fn after(duration: Duration) -> Self {
        Self {
            expires_at: Instant::now() + duration,
        }
    }

    pub fn from_instant(expires_at: Instant) -> Self {
        Self { expires_at }
    }

    pub fn child(&self, requested: Duration) -> Self {
        let requested_deadline = Instant::now() + requested;
        Self {
            expires_at: self.expires_at.min(requested_deadline),
        }
    }

    pub fn remaining(&self) -> Duration {
        self.expires_at.saturating_duration_since(Instant::now())
    }

    pub fn is_expired(&self) -> bool {
        self.remaining().is_zero()
    }

    pub fn instant(&self) -> Instant {
        self.expires_at
    }
}

#[derive(Clone)]
pub struct EmergencyRecorder {
    capacity: usize,
    entries: Arc<Mutex<VecDeque<String>>>,
}

impl EmergencyRecorder {
    pub fn new(capacity: usize) -> Result<Self, RuntimeError> {
        if capacity == 0 || capacity > 10_000 {
            return Err(RuntimeError::InvalidConfiguration);
        }
        Ok(Self {
            capacity,
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
        })
    }

    pub fn record(&self, stable_code: &str) -> bool {
        let valid = !stable_code.is_empty()
            && stable_code.len() <= 128
            && stable_code.bytes().all(|byte| {
                byte.is_ascii_uppercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
            });
        if !valid {
            return false;
        }
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if entries.len() == self.capacity {
            entries.pop_front();
        }
        entries.push_back(stable_code.to_owned());
        true
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_and_child_deadlines_inherit_parent_bounds() {
        let token = CancellationToken::root();
        let child = token.child_token();
        let waiter = tokio::spawn({
            let child = child.clone();
            async move { child.cancelled_reason().await }
        });
        token.cancel_with(CancellationReason::ModuleStop);
        assert_eq!(
            waiter.await.expect("waiter completes"),
            CancellationReason::ModuleStop
        );
        assert!(token.is_cancelled());
        assert!(child.is_cancelled());

        let parent = Deadline::after(Duration::from_millis(30));
        let child = parent.child(Duration::from_secs(1));
        assert_eq!(child.instant(), parent.instant());
    }

    #[test]
    fn emergency_ring_is_bounded_and_rejects_untrusted_text() {
        let recorder = EmergencyRecorder::new(2).expect("valid ring");
        assert!(recorder.record("FORGE.RUNTIME.STARTING"));
        assert!(recorder.record("FORGE.RUNTIME.DEGRADED"));
        assert!(recorder.record("FORGE.RUNTIME.READY"));
        assert!(!recorder.record("path=C:\\secret"));
        assert_eq!(
            recorder.snapshot(),
            vec!["FORGE.RUNTIME.DEGRADED", "FORGE.RUNTIME.READY"]
        );
    }

    #[test]
    fn runtime_rejects_unbounded_thread_settings() {
        assert!(
            KernelRuntime::new(RuntimeConfig {
                worker_threads: 0,
                ..RuntimeConfig::default()
            })
            .is_err()
        );
    }
}
