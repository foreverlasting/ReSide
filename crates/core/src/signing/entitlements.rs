//! Per-method entitlements filter (free Apple ID strips push, iCloud, IAP,
//! Associated Domains, …; paid cert preserves). Phase 2.
//!
//! ⚠️ **PARKED** — used by the abandoned native-signing pipeline; the live
//! app's signer handles entitlements through the fork. See [`super`] and
//! [`crate::signer`].
//!
//! Free-tier Apple IDs cannot be granted most "premium" capabilities. If a
//! re-signed app keeps an entitlement the free certificate is not provisioned
//! for, installation is rejected. So on the free path we strip those keys; on
//! the paid path we keep whatever the provisioning profile authorizes.

use super::SigningMethod;
use plist::Dictionary;

/// Entitlement keys (exact match) that a free Apple ID cannot use.
const FREE_DENY_EXACT: &[&str] = &[
    "aps-environment",                        // push notifications
    "com.apple.developer.in-app-payments",    // Apple Pay
    "com.apple.developer.associated-domains", // universal links
    "com.apple.developer.siri",
    "com.apple.developer.healthkit",
    "com.apple.developer.homekit",
    "com.apple.developer.networking.networkextension",
    "com.apple.developer.networking.vpn.api",
    "com.apple.developer.networking.multipath",
    "com.apple.developer.usernotifications.communication",
    "com.apple.developer.applesignin",
    "com.apple.security.application-groups", // app groups
    "inter-app-audio",
];

/// Entitlement key *prefixes* that a free Apple ID cannot use (covers the whole
/// iCloud / CloudKit family and other namespaced capabilities).
const FREE_DENY_PREFIX: &[&str] = &[
    "com.apple.developer.icloud-",
    "com.apple.developer.ubiquity-",
    "com.apple.developer.passkit",
    "com.apple.developer.payment-pass",
];

/// Result of filtering an entitlements dictionary.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterResult {
    /// The entitlements to actually sign with.
    pub kept: Dictionary,
    /// Keys that were removed (for surfacing `EntitlementsUnsupported` in the UI).
    pub stripped: Vec<String>,
}

fn is_denied_on_free(key: &str) -> bool {
    FREE_DENY_EXACT.contains(&key) || FREE_DENY_PREFIX.iter().any(|p| key.starts_with(p))
}

/// Filter an entitlements dictionary for the given signing method.
///
/// - [`SigningMethod::PaidCert`] preserves every entitlement (the profile
///   authorizes them).
/// - [`SigningMethod::FreeAppleId`] removes the keys a free Apple ID cannot be
///   provisioned for, returning them in `stripped`.
pub fn filter(entitlements: &Dictionary, method: SigningMethod) -> FilterResult {
    match method {
        SigningMethod::PaidCert => FilterResult {
            kept: entitlements.clone(),
            stripped: Vec::new(),
        },
        SigningMethod::FreeAppleId => {
            let mut kept = Dictionary::new();
            let mut stripped = Vec::new();
            for (key, value) in entitlements.iter() {
                if is_denied_on_free(key) {
                    stripped.push(key.clone());
                } else {
                    kept.insert(key.clone(), value.clone());
                }
            }
            stripped.sort();
            FilterResult { kept, stripped }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plist::Value;

    fn sample() -> Dictionary {
        let mut d = Dictionary::new();
        d.insert("application-identifier".into(), Value::from("ABCDE.com.x"));
        d.insert("get-task-allow".into(), Value::from(true));
        d.insert("aps-environment".into(), Value::from("production"));
        d.insert(
            "com.apple.developer.icloud-services".into(),
            Value::from("CloudKit"),
        );
        d.insert(
            "com.apple.security.application-groups".into(),
            Value::Array(vec![Value::from("group.com.x")]),
        );
        d
    }

    #[test]
    fn paid_preserves_everything() {
        let r = filter(&sample(), SigningMethod::PaidCert);
        assert!(r.stripped.is_empty());
        assert_eq!(r.kept.len(), 5);
    }

    #[test]
    fn free_strips_premium_keys_only() {
        let r = filter(&sample(), SigningMethod::FreeAppleId);
        // Basics survive.
        assert!(r.kept.contains_key("application-identifier"));
        assert!(r.kept.contains_key("get-task-allow"));
        // Premium keys are gone.
        assert!(!r.kept.contains_key("aps-environment"));
        assert!(!r.kept.contains_key("com.apple.developer.icloud-services"));
        assert!(!r.kept.contains_key("com.apple.security.application-groups"));
        assert_eq!(
            r.stripped,
            vec![
                "aps-environment",
                "com.apple.developer.icloud-services",
                "com.apple.security.application-groups",
            ]
        );
    }

    #[test]
    fn free_with_no_premium_keys_strips_nothing() {
        let mut d = Dictionary::new();
        d.insert("get-task-allow".into(), Value::from(true));
        let r = filter(&d, SigningMethod::FreeAppleId);
        assert!(r.stripped.is_empty());
        assert_eq!(r.kept.len(), 1);
    }
}
