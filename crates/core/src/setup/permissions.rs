//! System checks with copyable fix commands: usbmuxd, udev rules, group
//! membership, notification daemon, keyring, Developer Mode, ADI libs. Phase 1.
//!
//! This is the Phase 1 seed: a few cheap, real, side-effect-free probes so the
//! first-run screen shows live data. The remaining checks (udev rules, group
//! membership, Developer Mode, ADI libs) land as Phase 1/2 proceeds.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    Warn,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupCheck {
    pub key: String,
    pub label: String,
    pub status: CheckStatus,
    pub detail: String,
}

impl SetupCheck {
    fn new(key: &str, label: &str, ok: bool, ok_detail: &str, warn_detail: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            status: if ok {
                CheckStatus::Ok
            } else {
                CheckStatus::Warn
            },
            detail: if ok {
                ok_detail.to_string()
            } else {
                warn_detail.to_string()
            },
        }
    }
}

/// Run the currently-implemented checks. `keyring_available` is passed in by the
/// caller (the app already probes the Secret Service when building its store).
pub fn run_checks(keyring_available: bool) -> Vec<SetupCheck> {
    vec![
        SetupCheck::new(
            "usbmuxd",
            "usbmuxd",
            binary_on_path("usbmuxd"),
            "found on PATH",
            "not found — install the usbmuxd package",
        ),
        SetupCheck::new(
            "libimobiledevice",
            "libimobiledevice tools",
            binary_on_path("idevice_id"),
            "idevice_id found on PATH",
            "not found — install libimobiledevice",
        ),
        SetupCheck::new(
            "keyring",
            "Secret Service keyring",
            keyring_available,
            "available — credentials stored in your keyring",
            "not found — install gnome-keyring or KWallet to sign in (ReSide won't store your Apple password without one)",
        ),
        SetupCheck::new(
            "notifications",
            "Desktop notifications",
            std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some(),
            "session D-Bus reachable",
            "no session D-Bus — notifications may not fire",
        ),
    ]
}

/// True if `name` is an executable file in any `PATH` directory.
fn binary_on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_checks_returns_all_probes_with_keyring_reflected() {
        let with = run_checks(true);
        assert_eq!(with.len(), 4);
        let keyring = with.iter().find(|c| c.key == "keyring").unwrap();
        assert_eq!(keyring.status, CheckStatus::Ok);

        let without = run_checks(false);
        let keyring = without.iter().find(|c| c.key == "keyring").unwrap();
        assert_eq!(keyring.status, CheckStatus::Warn);
    }

    #[test]
    fn binary_on_path_finds_a_ubiquitous_binary() {
        // `sh` exists on any POSIX dev/CI box.
        assert!(binary_on_path("sh"));
        assert!(!binary_on_path("definitely-not-a-real-binary-xyzzy"));
    }
}
