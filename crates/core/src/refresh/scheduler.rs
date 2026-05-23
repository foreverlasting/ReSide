//! Background job loop: profile-refresh vs cert-refresh paths, idempotent with
//! retry/backoff. A failed refresh must never delete the currently installed
//! app unless re-install succeeds. Phase 4.
