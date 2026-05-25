//! Expiration tracking + background refresh.
//!
//! Two distinct job kinds with very different cost profiles — keep them
//! separate: `refresh_profile` (7-day, no Apple auth if anisette is fresh) and
//! `refresh_cert` (~1-year, full re-auth). Phase 4.

pub mod agent;
pub mod scheduler;

pub use agent::{AgentConfig, AgentMechanism, AgentMode, AgentStatus, DEFAULT_INTERVAL_HOURS};
pub use scheduler::{
    due_installs, refresh_due, refresh_installation, DueInstall, RefreshOutcome, RefreshReport,
    RefreshSummary, REFRESH_LEAD_SECS,
};
