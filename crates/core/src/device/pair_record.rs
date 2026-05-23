//! App-managed pair records stored under `$XDG_DATA_HOME/reside/pair_records/`
//! so first-run never requires root (plan.md §Permissions & Setup). Phase 1.
//!
//! We deliberately do not write to usbmuxd's cache or `/var/lib/lockdown/`;
//! records here are owned by ReSide and used to open trusted lockdown sessions.

use crate::error::{AppError, Result};
use idevice::pairing_file::PairingFile;
use std::path::{Path, PathBuf};

/// A directory of `<udid>.plist` pairing records.
pub struct PairRecordStore {
    dir: PathBuf,
}

impl PairRecordStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path_for(&self, udid: &str) -> PathBuf {
        self.dir.join(format!("{udid}.plist"))
    }

    /// True if a pairing record already exists for this device.
    pub fn exists(&self, udid: &str) -> bool {
        self.path_for(udid).is_file()
    }

    /// Serialize the pairing record to PLIST and persist it with owner-only
    /// permissions. The record carries private key material, so it is treated
    /// like a secret on disk (0600).
    pub fn save(&self, udid: &str, record: &PairingFile) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let bytes = record
            .clone()
            .serialize()
            .map_err(|e| AppError::Internal(format!("serialize pairing file: {e}")))?;
        let path = self.path_for(udid);
        std::fs::write(&path, &bytes)?;
        set_owner_only(&path)?;
        Ok(())
    }

    /// Load a previously saved pairing record (e.g. to start a trusted session).
    pub fn load(&self, udid: &str) -> Result<PairingFile> {
        PairingFile::read_from_file(self.path_for(udid))
            .map_err(|e| AppError::Internal(format!("read pairing file: {e}")))
    }

    /// Remove a pairing record. No-op if it does not exist.
    pub fn delete(&self, udid: &str) -> Result<()> {
        let path = self.path_for(udid);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_path_uses_udid_filename() {
        let store = PairRecordStore::new("/tmp/reside-test");
        assert!(store
            .path_for("00008110-0001")
            .ends_with("00008110-0001.plist"));
    }

    #[test]
    fn exists_and_delete_track_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let store = PairRecordStore::new(tmp.path());
        let udid = "abc123";
        assert!(!store.exists(udid));

        // A pairing record on disk is opaque bytes to the store; the storage
        // plumbing (exists/delete) is independent of PairingFile parsing.
        std::fs::write(store.path_for(udid), b"<plist/>").unwrap();
        assert!(store.exists(udid));

        store.delete(udid).unwrap();
        assert!(!store.exists(udid));
        // Deleting a missing record is a no-op.
        store.delete(udid).unwrap();
    }
}
