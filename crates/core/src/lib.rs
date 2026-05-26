//! ReSide core: orchestration, signing, device transport, install, and refresh.
//!
//! The UI (Tauri) and the background agent both depend on this crate. Keep it
//! free of Tauri- and UI-specific concerns so a future CLI binary can reuse it.
//!
//! Phase status: 0b primitives are implemented (error taxonomy, operation
//! events, paths, db + migrations, secure storage, process lock, IPA store).
//! Domain modules (device/transport/signing/installer/refresh/setup) are stubs
//! that land with their respective phases.

pub mod db;
pub mod error;
pub mod installs;
pub mod ipa_meta;
pub mod ipa_store;
pub mod locate;
pub mod operation;
pub mod paths;
pub mod proc_lock;
pub mod secure_storage;

pub mod device;
pub mod installer;
pub mod refresh;
pub mod setup;
pub mod signer;
pub mod signing;
pub mod transport;

pub use error::{AppError, ErrorCategory, ErrorReport, Redactable, Result, Secret};
pub use operation::{Operation, OperationEvent, OperationStage};
