//! Locating the external helper binaries ReSide *spawns* rather than links: the
//! D `sideloader` signer and the `netmuxd` Wi-Fi bridge. Both are separate
//! processes (the signer is GPL D, netmuxd is LGPL) found at runtime, not
//! compiled in.
//!
//! ## Resolution order
//! 1. An explicit env override (`RESIDE_SIDELOADER_BIN` / `RESIDE_NETMUXD_BIN`) —
//!    how a dev checkout points at hand-built binaries, and an escape hatch for
//!    any install.
//! 2. A binary of the given `name` sitting **next to the running executable** —
//!    where a packaged build ships its sidecars, so an install needs no
//!    configuration and no hardcoded `/home/...` path.
//! 3. Bare `name`, left for the OS to resolve on `PATH`.
//!
//! Returning an *absolute* path from step 1 or 2 matters downstream: the
//! background agent only bakes a resolved helper path into its systemd unit when
//! it's absolute (a unit starts with an empty environment and can't reproduce a
//! bare `PATH` lookup). So "beside the executable" is what makes an installed
//! build's unattended agent self-configure.

use std::path::{Path, PathBuf};

/// Resolve a spawned helper binary by the order documented on this module:
/// `env_var` override → `name` beside the current executable → bare `name`.
pub fn helper_binary(env_var: &str, name: &str) -> PathBuf {
    std::env::var_os(env_var)
        .map(PathBuf::from)
        .or_else(|| beside_current_exe(name))
        .unwrap_or_else(|| PathBuf::from(name))
}

/// `name` if it exists alongside the running executable, else `None`. This is
/// where Tauri-style sidecar bundling drops external binaries.
fn beside_current_exe(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    beside(exe.parent()?, name)
}

/// `dir/name` if it's an existing file. Split out from [`beside_current_exe`] so
/// the existence logic is unit-testable without depending on the test runner's
/// own path.
fn beside(dir: &Path, name: &str) -> Option<PathBuf> {
    let candidate = dir.join(name);
    candidate.is_file().then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENV: &str = "RESIDE_LOCATE_TEST_BIN";

    #[test]
    fn env_override_wins_and_is_taken_verbatim() {
        std::env::set_var(ENV, "/opt/custom/thing");
        assert_eq!(
            helper_binary(ENV, "thing"),
            PathBuf::from("/opt/custom/thing")
        );
        std::env::remove_var(ENV);
    }

    #[test]
    fn falls_back_to_bare_name_when_nothing_resolves() {
        std::env::remove_var(ENV);
        // No env override and (in the test runner's dir) no sibling named this,
        // so we get the bare name for the OS to resolve on PATH.
        assert_eq!(
            helper_binary(ENV, "reside-nonexistent-helper-xyz"),
            PathBuf::from("reside-nonexistent-helper-xyz")
        );
    }

    #[test]
    fn beside_finds_an_existing_sibling_only() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(beside(dir.path(), "helper"), None);

        let path = dir.path().join("helper");
        std::fs::write(&path, b"#!/bin/sh\n").unwrap();
        assert_eq!(beside(dir.path(), "helper"), Some(path));
    }
}
