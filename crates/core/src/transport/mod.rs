//! Device transport abstraction.
//!
//! v1 ships a single `RemoteXpc` strategy (RSD / RemoteXPC for iOS 17.4+). The
//! module is built around a `Transport` trait from day one so a `LegacyLockdown`
//! strategy can be contributed later without restructuring. Phase 1.

pub mod mdns_discovery;
pub mod remote_xpc;
pub mod tunneld;
