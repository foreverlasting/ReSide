//! ReSide core: orchestration, signing, device transport, install, and refresh.
//!
//! The UI (Tauri) and the background agent both depend on this crate. Keep it
//! free of Tauri- and UI-specific concerns so a future CLI binary can reuse it.
//!
//! Implemented and hardware-validated: sign + install over USB and Wi-Fi,
//! auto-refresh engine + unattended agent, 3-tier credentials.
//!
//! LIVE vs PARKED (see `docs/ARCHITECTURE.md`): the live signing path is
//! [`signer`], which drives the forked Sideloader CLI. The [`signing`] module
//! and [`setup::adi_provision`] are the **superseded native attempt** — still
//! compiled but not wired into the live app (the Tauri backend uses `signer`,
//! never `signing`). Their comments may still say "Phase 2/future"; that is
//! historical, not a roadmap.

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
