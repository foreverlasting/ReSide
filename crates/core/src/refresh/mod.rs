//! Expiration tracking + background refresh.
//!
//! Two distinct job kinds with very different cost profiles — keep them
//! separate: `refresh_profile` (7-day, no Apple auth if anisette is fresh) and
//! `refresh_cert` (~1-year, full re-auth). Phase 4.

pub mod agent;
pub mod scheduler;
