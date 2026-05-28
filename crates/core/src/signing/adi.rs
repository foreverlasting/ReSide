//! Local anisette via Apple's ADI libraries.
//!
//! ⚠️ **PARKED** — used only by the abandoned native-signing path. The live
//! app's anisette/ADI provisioning runs inside the forked Sideloader (which
//! downloads the Apple Music APK at first sign-in). See [`super`] and
//! [`crate::signer`].
//!
//! `omnisette` generates anisette data from Apple's proprietary `libCoreADI.so`
//! / `libstoreservicescore.so`, loaded and executed at runtime through Dadoum's
//! `android-loader` (the free-signing path is therefore **not** pure Rust — see
//! plan.md §Anisette & Apple ADI libraries; do not try to remove the FFI). The
//! libraries are placed on disk by [`crate::setup::adi_provision`].
//!
//! **No-remote-fallback policy.** `omnisette::get_anisette_headers_provider`
//! silently falls back to a *remote* anisette server when the local provider
//! fails to load. ReSide rejects that (no third party in the auth loop, no
//! account-lock risk from shared anisette state). So before the free-Apple-ID
//! flow ever constructs an account, [`require_provisioned`] guarantees the local
//! libraries are present, and the flow must point `icloud_auth` at the same
//! [`anisette_config`] dir so the local provider — not the remote one — is used.

use crate::error::{AppError, Result};
use crate::setup::adi_provision;
use omnisette::{AnisetteConfiguration, AnisetteHeaders};
use std::path::Path;

/// Build the `omnisette` configuration pointed at ReSide's ADI directory. The
/// synthetic-device fingerprint and provisioning state persist under this path;
/// it must be unique per install and never copied between machines (account-lock
/// hazard — plan.md §Known Foot-Guns).
pub fn anisette_config(adi_dir: &Path) -> AnisetteConfiguration {
    AnisetteConfiguration::new().set_configuration_path(adi_dir.to_path_buf())
}

/// Whether the ADI libraries have been provisioned for this host.
pub fn provisioned(adi_dir: &Path) -> bool {
    adi_provision::libs_present(adi_dir)
}

/// Fail with [`AppError::AnisetteAdiUnavailable`] if the ADI libraries are not
/// yet present. Cheap, no FFI — call this as the first gate in the credential
/// flow so a missing library surfaces the "run setup" remediation rather than a
/// silent slide into remote anisette.
pub fn require_provisioned(adi_dir: &Path) -> Result<()> {
    if provisioned(adi_dir) {
        Ok(())
    } else {
        Err(AppError::AnisetteAdiUnavailable)
    }
}

/// Confirm the ADI libraries actually *load* (via `android-loader`), not just
/// that the files exist. Loads the local store-services-core provider and maps
/// failures to a clean taxonomy:
/// - libraries absent → [`AppError::AnisetteAdiUnavailable`]
/// - present but unloadable / wrong ABI → [`AppError::AnisetteAdiIncompatible`]
///
/// Touches the FFI boundary (and `omnisette` may begin synthetic-device
/// provisioning), so this is a setup-time verification step, not a hot path.
pub fn verify_local_provider(adi_dir: &Path) -> Result<()> {
    require_provisioned(adi_dir)?;
    match AnisetteHeaders::get_ssc_anisette_headers_provider(anisette_config(adi_dir)) {
        Ok(res) => {
            // Force local: never accept a provider that resolved to remote.
            if matches!(
                res.provider_type,
                omnisette::AnisetteHeadersProviderType::Local
            ) {
                Ok(())
            } else {
                Err(AppError::AnisetteAdiIncompatible)
            }
        }
        Err(e) => {
            tracing::warn!(target: "reside::adi", error = %e, "local ADI provider failed to load");
            Err(AppError::AnisetteAdiIncompatible)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_provisioned_errors_when_libs_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let adi = tmp.path().join("adi");
        assert!(!provisioned(&adi));
        let err = require_provisioned(&adi).unwrap_err();
        assert!(matches!(err, AppError::AnisetteAdiUnavailable));
    }

    #[test]
    fn anisette_config_points_at_adi_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let adi = tmp.path().join("adi");
        let cfg = anisette_config(&adi);
        assert_eq!(cfg.configuration_path(), &adi);
    }
}
