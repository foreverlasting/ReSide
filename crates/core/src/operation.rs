//! Transport-agnostic operation event channel.
//!
//! Long-running flows (sign, install, refresh) emit [`OperationEvent`]s as they
//! progress. The Tauri layer subscribes and re-broadcasts them as `operation_{id}`
//! events; a future CLI could subscribe and render a progress bar instead. Core
//! never depends on Tauri.

use crate::error::{AppError, ErrorReport};
use tokio::sync::broadcast;

/// Stages mirror the frontend `OperationEvent.stage` union in plan.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStage {
    Queued,
    Preparing,
    Authenticating,
    Awaiting2fa,
    Signing,
    Transferring,
    Installing,
    Verifying,
    TrustRequired,
    Done,
    Failed,
}

/// A single progress event for one operation. `message` must already be
/// redacted by the caller; `error` is the UI-safe report (never raw error text).
#[derive(Debug, Clone, serde::Serialize)]
pub struct OperationEvent {
    pub id: String,
    pub stage: OperationStage,
    pub progress: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorReport>,
}

/// A multi-producer broadcast hub. Clone freely; subscribers created before an
/// event is sent will receive it.
#[derive(Clone)]
pub struct OperationChannel {
    tx: broadcast::Sender<OperationEvent>,
}

impl OperationChannel {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(256);
        Self { tx }
    }

    /// Subscribe to all subsequent events across every operation.
    pub fn subscribe(&self) -> broadcast::Receiver<OperationEvent> {
        self.tx.subscribe()
    }

    /// Begin a new operation with the given id, emitting a `Queued` event.
    pub fn start(&self, id: impl Into<String>) -> Operation {
        let op = Operation {
            id: id.into(),
            tx: self.tx.clone(),
        };
        op.emit(OperationStage::Queued, 0.0, None, None);
        op
    }
}

impl Default for OperationChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle for emitting progress on a single operation.
#[derive(Clone)]
pub struct Operation {
    id: String,
    tx: broadcast::Sender<OperationEvent>,
}

impl Operation {
    pub fn id(&self) -> &str {
        &self.id
    }

    fn emit(
        &self,
        stage: OperationStage,
        progress: f32,
        message: Option<String>,
        error: Option<ErrorReport>,
    ) {
        // A send error only means there are no live subscribers — that's fine.
        let _ = self.tx.send(OperationEvent {
            id: self.id.clone(),
            stage,
            progress: progress.clamp(0.0, 1.0),
            message,
            error,
        });
    }

    /// Advance to `stage` with the given fractional progress (0.0–1.0).
    pub fn stage(&self, stage: OperationStage, progress: f32, message: Option<String>) {
        self.emit(stage, progress, message, None);
    }

    /// Terminal success.
    pub fn done(&self) {
        self.emit(OperationStage::Done, 1.0, None, None);
    }

    /// Terminal failure, carrying a redacted error report.
    pub fn fail(&self, err: &AppError) {
        self.emit(OperationStage::Failed, 1.0, None, Some(err.report()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subscriber_receives_lifecycle_events() {
        let chan = OperationChannel::new();
        let mut rx = chan.subscribe();
        let op = chan.start("op-1");
        op.stage(OperationStage::Signing, 0.5, Some("signing".into()));
        op.done();

        let queued = rx.recv().await.unwrap();
        assert_eq!(queued.stage, OperationStage::Queued);
        assert_eq!(queued.id, "op-1");

        let signing = rx.recv().await.unwrap();
        assert_eq!(signing.stage, OperationStage::Signing);
        assert_eq!(signing.progress, 0.5);

        let done = rx.recv().await.unwrap();
        assert_eq!(done.stage, OperationStage::Done);
    }

    #[tokio::test]
    async fn fail_carries_redacted_report() {
        let chan = OperationChannel::new();
        let mut rx = chan.subscribe();
        let op = chan.start("op-2");
        op.fail(&AppError::DeviceLocked);

        let _queued = rx.recv().await.unwrap();
        let failed = rx.recv().await.unwrap();
        assert_eq!(failed.stage, OperationStage::Failed);
        assert_eq!(failed.error.unwrap().category, "DeviceLocked");
    }
}
