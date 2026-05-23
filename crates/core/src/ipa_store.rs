//! Content-addressed IPA store. The original IPA must be retained so background
//! refresh can re-sign without the user re-importing (see plan.md notes on
//! `apps.source_ipa_path`). Files are named `<sha256>.ipa`, which also dedups
//! identical imports across apps.

use crate::error::Result;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredIpa {
    pub sha256: String,
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct IpaStore {
    dir: PathBuf,
}

impl IpaStore {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
        }
    }

    pub fn path_for(&self, sha256: &str) -> PathBuf {
        self.dir.join(format!("{sha256}.ipa"))
    }

    pub fn contains(&self, sha256: &str) -> bool {
        self.path_for(sha256).is_file()
    }

    /// Copy an external IPA into the store, addressed by its content hash.
    /// If an identical file is already present, this is a no-op copy.
    pub fn store_file(&self, src: impl AsRef<Path>) -> Result<StoredIpa> {
        let mut file = std::fs::File::open(&src)?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let sha256 = hex::encode(hasher.finalize());
        let dest = self.path_for(&sha256);
        if !dest.is_file() {
            std::fs::create_dir_all(&self.dir)?;
            std::fs::copy(&src, &dest)?;
        }
        let size = std::fs::metadata(&dest)?.len();
        Ok(StoredIpa {
            sha256,
            path: dest,
            size,
        })
    }

    /// Store an in-memory IPA payload, addressed by its content hash.
    pub fn store_bytes(&self, bytes: &[u8]) -> Result<StoredIpa> {
        let sha256 = hex::encode(Sha256::digest(bytes));
        let dest = self.path_for(&sha256);
        if !dest.is_file() {
            std::fs::create_dir_all(&self.dir)?;
            std::fs::write(&dest, bytes)?;
        }
        Ok(StoredIpa {
            sha256,
            path: dest,
            size: bytes.len() as u64,
        })
    }

    /// Remove a stored IPA by hash (used by retention sweeps once no
    /// installation references it). Missing is treated as success.
    pub fn remove(&self, sha256: &str) -> Result<()> {
        match std::fs::remove_file(self.path_for(sha256)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_bytes_is_content_addressed_and_dedups() {
        let tmp = tempfile::tempdir().unwrap();
        let store = IpaStore::new(tmp.path().join("ipas"));

        let a = store.store_bytes(b"fake ipa payload").unwrap();
        assert!(store.contains(&a.sha256));
        assert_eq!(
            a.path.file_name().unwrap().to_str().unwrap(),
            format!("{}.ipa", a.sha256)
        );
        assert_eq!(a.size, 16);

        // Identical content → identical address, still present, no error.
        let b = store.store_bytes(b"fake ipa payload").unwrap();
        assert_eq!(a, b);

        // Different content → different address.
        let c = store.store_bytes(b"another payload").unwrap();
        assert_ne!(a.sha256, c.sha256);
    }

    #[test]
    fn store_file_matches_store_bytes_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let store = IpaStore::new(tmp.path().join("ipas"));
        let src = tmp.path().join("in.ipa");
        std::fs::write(&src, b"hello reside").unwrap();

        let viafile = store.store_file(&src).unwrap();
        let viabytes_hash = {
            let s2 = IpaStore::new(tmp.path().join("ipas2"));
            s2.store_bytes(b"hello reside").unwrap().sha256
        };
        assert_eq!(viafile.sha256, viabytes_hash);

        store.remove(&viafile.sha256).unwrap();
        assert!(!store.contains(&viafile.sha256));
    }
}
