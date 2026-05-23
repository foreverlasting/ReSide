//! XDG path resolution. All on-disk locations funnel through here — never
//! hardcode `~` paths elsewhere (see plan.md §Filesystem Layout).

use crate::error::{AppError, Result};
use std::path::{Path, PathBuf};

const APP_DIR: &str = "reside";

/// Resolved base directories for the app. Use [`Paths::resolve`] in production
/// or [`Paths::with_root`] in tests to sandbox everything under one directory.
#[derive(Debug, Clone)]
pub struct Paths {
    data: PathBuf,
    config: PathBuf,
    state: PathBuf,
    runtime: Option<PathBuf>,
}

impl Paths {
    /// Resolve from the real XDG environment via the `dirs` crate.
    pub fn resolve() -> Result<Self> {
        let data = dirs::data_dir()
            .ok_or_else(|| AppError::Internal("XDG data dir unavailable".into()))?
            .join(APP_DIR);
        let config = dirs::config_dir()
            .ok_or_else(|| AppError::Internal("XDG config dir unavailable".into()))?
            .join(APP_DIR);
        // `state_dir` is None on some platforms; fall back to data dir.
        let state = dirs::state_dir()
            .unwrap_or_else(|| data.clone())
            .join(APP_DIR);
        let runtime = dirs::runtime_dir().map(|r| r.join(APP_DIR));
        Ok(Self {
            data,
            config,
            state,
            runtime,
        })
    }

    /// Sandbox every base under a single root (for tests / portable mode).
    pub fn with_root(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self {
            data: root.join("data"),
            config: root.join("config"),
            state: root.join("state"),
            runtime: Some(root.join("runtime")),
        }
    }

    // ---- base dirs ----
    pub fn data_dir(&self) -> &Path {
        &self.data
    }
    pub fn config_dir(&self) -> &Path {
        &self.config
    }
    pub fn state_dir(&self) -> &Path {
        &self.state
    }
    pub fn runtime_dir(&self) -> Option<&Path> {
        self.runtime.as_deref()
    }

    // ---- data dir contents ----
    pub fn database_file(&self) -> PathBuf {
        self.data.join("data.db")
    }
    pub fn ipa_store_dir(&self) -> PathBuf {
        self.data.join("ipas")
    }
    pub fn pair_records_dir(&self) -> PathBuf {
        self.data.join("pair_records")
    }
    pub fn profiles_dir(&self) -> PathBuf {
        self.data.join("profiles")
    }
    pub fn adi_dir(&self) -> PathBuf {
        self.data.join("adi")
    }
    pub fn logs_dir(&self) -> PathBuf {
        self.data.join("logs")
    }

    // ---- config dir contents ----
    pub fn config_file(&self) -> PathBuf {
        self.config.join("config.toml")
    }

    // ---- state dir contents ----
    pub fn agent_pid_file(&self) -> PathBuf {
        self.state.join("agent.pid")
    }
    pub fn tunneld_socket(&self) -> PathBuf {
        self.state.join("tunneld.sock")
    }

    /// Create every directory the app writes into. Idempotent.
    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [
            &self.data,
            &self.config,
            &self.state,
            &self.ipa_store_dir(),
            &self.pair_records_dir(),
            &self.profiles_dir(),
            &self.adi_dir(),
            &self.logs_dir(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        if let Some(rt) = &self.runtime {
            // Runtime dir may be unavailable / non-writable; treat as best-effort.
            let _ = std::fs::create_dir_all(rt);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_root_lays_out_all_paths_and_ensure_dirs_creates_them() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::with_root(tmp.path());
        paths.ensure_dirs().unwrap();

        assert!(paths.data_dir().is_dir());
        assert!(paths.ipa_store_dir().is_dir());
        assert!(paths.adi_dir().is_dir());
        assert!(paths.logs_dir().is_dir());
        assert!(paths.state_dir().is_dir());
        assert_eq!(paths.database_file().file_name().unwrap(), "data.db");
        assert_eq!(paths.tunneld_socket().file_name().unwrap(), "tunneld.sock");
    }
}
