//! Single-writer advisory file lock shared between the UI and the background
//! agent (see plan.md §Process coordination). Whichever process holds the lock
//! at `$XDG_STATE_HOME/reside/agent.pid` is the one allowed to mutate state.

use crate::error::Result;
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

/// An held exclusive lock. The OS releases it when this value is dropped (the
/// underlying file descriptor closes).
pub struct ProcLock {
    file: File,
}

impl ProcLock {
    /// Try to acquire the lock without blocking. Returns `Ok(None)` if another
    /// process already holds it.
    pub fn try_acquire(path: impl AsRef<Path>) -> Result<Option<Self>> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)?;

        match file.try_lock_exclusive() {
            Ok(()) => {
                let mut f = &file;
                // Record our pid for diagnostics; not used for correctness.
                let _ = f.set_len(0);
                let _ = write!(f, "{}", std::process::id());
                let _ = f.flush();
                Ok(Some(Self { file }))
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Explicitly release the lock (also happens on drop).
    pub fn release(self) {
        // Call the fs2 trait method explicitly: std's inherent `File::unlock`
        // is only stable since 1.89, above our MSRV floor.
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_is_refused_while_first_is_held() {
        let tmp = tempfile::tempdir().unwrap();
        let lock_path = tmp.path().join("state").join("agent.pid");

        let first = ProcLock::try_acquire(&lock_path).unwrap();
        assert!(first.is_some(), "first acquire should win");

        // A second, independent handle to the same file represents another
        // process; the OS-level advisory lock must refuse it.
        let second = ProcLock::try_acquire(&lock_path).unwrap();
        assert!(
            second.is_none(),
            "second acquire must be refused while held"
        );

        // After releasing the first, the lock becomes available again.
        first.unwrap().release();
        let third = ProcLock::try_acquire(&lock_path).unwrap();
        assert!(third.is_some(), "acquire should succeed after release");
    }

    #[test]
    fn pid_is_recorded_in_lock_file() {
        let tmp = tempfile::tempdir().unwrap();
        let lock_path = tmp.path().join("agent.pid");
        let lock = ProcLock::try_acquire(&lock_path).unwrap().unwrap();
        let contents = std::fs::read_to_string(&lock_path).unwrap();
        assert_eq!(contents.trim(), std::process::id().to_string());
        lock.release();
    }
}
