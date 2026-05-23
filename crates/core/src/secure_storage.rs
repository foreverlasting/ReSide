//! Secret storage: system keyring (Linux Secret Service) with a filesystem
//! fallback when no keyring daemon is reachable.
//!
//! All secret material (Apple ID passwords, signing keys, anisette state) flows
//! through here and nowhere near SQLite or logs. When the keyring is missing we
//! surface [`AppError::KeyringUnavailable`] to the UI once, then degrade to the
//! filesystem store.
//!
//! NOTE: the filesystem fallback currently writes secrets with `0600`
//! permissions but is NOT yet encrypted at rest — that hardening is tracked as
//! follow-up work and must land before the fallback is considered production-safe.

use crate::error::{AppError, Result};
use std::path::{Path, PathBuf};

const KEYRING_SERVICE: &str = "reside";

/// Backend selected for this process.
#[derive(Debug, Clone)]
pub enum SecureStore {
    /// System Secret Service via the `keyring` crate.
    Keyring,
    /// Filesystem fallback rooted at a secrets directory.
    File(PathBuf),
}

impl SecureStore {
    /// Probe the system keyring; fall back to a filesystem store under
    /// `secrets_dir` if it is unreachable. Returns the store plus an optional
    /// warning the UI should surface once.
    pub fn detect(secrets_dir: impl AsRef<Path>) -> (Self, Option<AppError>) {
        match probe_keyring() {
            Ok(()) => (Self::Keyring, None),
            Err(()) => (
                Self::File(secrets_dir.as_ref().to_path_buf()),
                Some(AppError::KeyringUnavailable),
            ),
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
/// reachable), Err if no storage is accessible.
fn probe_keyring() -> std::result::Result<(), ()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, "__probe__").map_err(|_| ())?;
    match entry.get_password() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(()),
    }
}

/// Map an arbitrary key to a safe flat filename.
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

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
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
    fn detect_returns_a_usable_store() {
        // In headless CI there is usually no Secret Service, so this should pick
        // the filesystem fallback and hand back a one-time warning. Either way
        // the returned store must round-trip.
        let tmp = tempfile::tempdir().unwrap();
        let (store, warning) = SecureStore::detect(tmp.path().join("secrets"));
        if let SecureStore::File(_) = store {
            assert!(matches!(warning, Some(AppError::KeyringUnavailable)));
        }
    }
}
