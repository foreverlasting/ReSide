//! Secret storage: the system keyring (Linux Secret Service) — and nothing else.
//!
//! All secret material (currently the Apple ID + password) flows through here and
//! nowhere near SQLite or logs. If no keyring daemon is reachable, ReSide
//! **refuses to store credentials** rather than fall back to a plaintext file: an
//! Apple password is never written to disk in the clear. Callers surface
//! [`AppError::KeyringUnavailable`] so the user installs a keyring
//! (gnome-keyring / KWallet) before signing in.
//!
//! A filesystem-backed [`SecureStore::File`] exists only under `cfg(test)`, as a
//! convenient real backend for unit tests; production builds cannot construct it.

use crate::error::{AppError, Result};
#[cfg(test)]
use std::path::{Path, PathBuf};

const KEYRING_SERVICE: &str = "reside";

/// Backend selected for this process.
#[derive(Debug, Clone)]
pub enum SecureStore {
    /// System Secret Service via the `keyring` crate — the only production backend.
    Keyring,
    /// No secure backend is available. Refuses to store secrets (so an Apple
    /// password is never written in plain text) and reports nothing stored.
    Unavailable,
    /// Test-only filesystem store. Never selected by [`SecureStore::detect`];
    /// present only so unit tests can round-trip secrets without a live keyring.
    #[cfg(test)]
    File(PathBuf),
}

impl SecureStore {
    /// Probe the system keyring. Returns [`SecureStore::Keyring`] when reachable,
    /// else [`SecureStore::Unavailable`] plus a one-time warning the UI surfaces —
    /// ReSide will not store credentials without a keyring.
    pub fn detect() -> (Self, Option<AppError>) {
        match probe_keyring() {
            Ok(()) => (Self::Keyring, None),
            Err(()) => (Self::Unavailable, Some(AppError::KeyringUnavailable)),
        }
    }

    pub fn set(&self, key: &str, secret: &str) -> Result<()> {
        match self {
            Self::Keyring => {
                let entry = keyring::Entry::new(KEYRING_SERVICE, key)
                    .map_err(|_| AppError::KeyringUnavailable)?;
                entry
                    .set_password(secret)
                    .map_err(|_| AppError::KeyringUnavailable)
            }
            // Refuse rather than persist a secret in the clear.
            Self::Unavailable => Err(AppError::KeyringUnavailable),
            #[cfg(test)]
            Self::File(dir) => {
                std::fs::create_dir_all(dir)?;
                let path = dir.join(file_name(key));
                std::fs::write(&path, secret.as_bytes())?;
                restrict_permissions(&path)?;
                Ok(())
            }
        }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        match self {
            Self::Keyring => {
                let entry = keyring::Entry::new(KEYRING_SERVICE, key)
                    .map_err(|_| AppError::KeyringUnavailable)?;
                match entry.get_password() {
                    Ok(v) => Ok(Some(v)),
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(_) => Err(AppError::KeyringUnavailable),
                }
            }
            // Nothing is ever stored without a keyring.
            Self::Unavailable => Ok(None),
            #[cfg(test)]
            Self::File(dir) => {
                let path = dir.join(file_name(key));
                match std::fs::read_to_string(&path) {
                    Ok(v) => Ok(Some(v)),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(e) => Err(e.into()),
                }
            }
        }
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        match self {
            Self::Keyring => {
                let entry = keyring::Entry::new(KEYRING_SERVICE, key)
                    .map_err(|_| AppError::KeyringUnavailable)?;
                match entry.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(_) => Err(AppError::KeyringUnavailable),
                }
            }
            // Nothing stored → nothing to delete.
            Self::Unavailable => Ok(()),
            #[cfg(test)]
            Self::File(dir) => {
                let path = dir.join(file_name(key));
                match std::fs::remove_file(&path) {
                    Ok(()) => Ok(()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(e) => Err(e.into()),
                }
            }
        }
    }
}

/// Returns Ok if a keyring backend is reachable (an empty probe counts as
/// reachable), Err if no keyring is accessible.
fn probe_keyring() -> std::result::Result<(), ()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, "__probe__").map_err(|_| ())?;
    match entry.get_password() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(()),
    }
}

/// Map an arbitrary key to a safe flat filename (test-only filesystem store).
#[cfg(test)]
fn file_name(key: &str) -> String {
    let mut s: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    s.push_str(".secret");
    s
}

#[cfg(all(test, unix))]
fn restrict_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(all(test, not(unix)))]
fn restrict_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_fallback_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SecureStore::File(tmp.path().join("secrets"));

        assert_eq!(store.get("reside.anisette.abc").unwrap(), None);
        store.set("reside.anisette.abc", "fingerprint-xyz").unwrap();
        assert_eq!(
            store.get("reside.anisette.abc").unwrap().as_deref(),
            Some("fingerprint-xyz")
        );

        store.delete("reside.anisette.abc").unwrap();
        assert_eq!(store.get("reside.anisette.abc").unwrap(), None);
        // Deleting a missing key is a no-op.
        store.delete("reside.anisette.abc").unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn file_fallback_uses_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("secrets");
        let store = SecureStore::File(dir.clone());
        store.set("k", "v").unwrap();
        let mode = std::fs::metadata(dir.join(file_name("k")))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn detect_never_falls_back_to_plaintext() {
        // CI is usually headless (no Secret Service), so this typically picks
        // Unavailable; a dev box with a keyring picks Keyring. Either way, detect
        // must NEVER hand back a plaintext filesystem store.
        let (store, warning) = SecureStore::detect();
        match store {
            SecureStore::Keyring => assert!(warning.is_none()),
            SecureStore::Unavailable => {
                assert!(matches!(warning, Some(AppError::KeyringUnavailable)));
                // The whole point: refuse to store, report nothing stored.
                assert!(matches!(
                    store.set("k", "v"),
                    Err(AppError::KeyringUnavailable)
                ));
                assert_eq!(store.get("k").unwrap(), None);
                store.delete("k").unwrap();
            }
            SecureStore::File(_) => unreachable!("detect must never select a plaintext store"),
        }
    }
}
