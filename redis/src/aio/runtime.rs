use std::{io, sync::Arc, time::Duration};
use std::future::Future;

use crate::types::RedisError;

use super::tokio as crate_tokio;

/// Represents the only supported runtime (Tokio).
#[derive(Clone, Copy, Debug)]
pub(crate) enum Runtime {
    Tokio,
}

/// Join-handle wrapper for spawned background tasks.
pub(crate) enum TaskHandle {
    Tokio(tokio::task::JoinHandle<()>),
}

/// Internal helper that aborts the task when the last clone is dropped.
struct HandleContainer(Option<TaskHandle>);

impl HandleContainer {
    fn new(handle: TaskHandle) -> Self {
        Self(Some(handle))
    }
}

impl Drop for HandleContainer {
    fn drop(&mut self) {
        if let Some(TaskHandle::Tokio(handle)) = self.0.take() {
            handle.abort();
        }
    }
}

#[derive(Clone)]
pub(crate) struct SharedHandleContainer(Arc<HandleContainer>);

impl SharedHandleContainer {
    pub(crate) fn new(handle: TaskHandle) -> Self {
        Self(Arc::new(HandleContainer::new(handle)))
    }
}

impl Runtime {
    /// Locate the currently-running runtime (always Tokio).
    pub(crate) fn locate() -> Self {
        Runtime::Tokio
    }

    /// Spawn a background future on the runtime.
    #[allow(dead_code)]
    pub(crate) fn spawn(&self, f: impl Future<Output = ()> + Send + 'static) -> TaskHandle {
        match self {
            Runtime::Tokio => TaskHandle::Tokio(tokio::spawn(f)),
        }
    }

    /// Run `future` with a timeout.
    pub(crate) async fn timeout<F: Future>(
        &self,
        duration: Duration,
        future: F,
    ) -> Result<F::Output, Elapsed> {
        match self {
            Runtime::Tokio => tokio::time::timeout(duration, future)
                .await
                .map_err(|_| Elapsed(())),
        }
    }

    #[cfg(any(feature = "connection-manager", feature = "cluster-async"))]
    pub(crate) async fn sleep(&self, duration: Duration) {
        match self {
            Runtime::Tokio => tokio::time::sleep(duration).await,
        }
    }

    #[cfg(feature = "cluster-async")]
    pub(crate) async fn locate_and_sleep(duration: Duration) {
        Self::locate().sleep(duration).await
    }
}

#[derive(Debug)]
pub(crate) struct Elapsed(());

impl From<Elapsed> for RedisError {
    fn from(_: Elapsed) -> Self {
        io::Error::from(io::ErrorKind::TimedOut).into()
    }
}
