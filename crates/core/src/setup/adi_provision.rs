//! One-time ADI library provisioning.
//!
//! ⚠️ **PARKED — superseded.** Nothing in `reside-app` calls this module: the
//! live app delegates ADI provisioning to the forked Sideloader, which fetches
//! the Apple Music APK from Apple's CDN at first sign-in. Kept for reference /
//! the `native-signing-path` branch. See [`crate::signer`].
//!
//! Generating anisette data locally requires two of Apple's proprietary native
//! libraries — `libstoreservicescore.so` and `libCoreADI.so` — which ship inside
//! the Apple Music **Android** APK. They are **non-redistributable**: ReSide must
//! never bundle or commit them (plan.md §Known Foot-Guns). Instead the user
//! supplies an Apple Music APK and this module extracts the two `.so` files into
//! the app's ADI directory in the on-disk layout `omnisette` expects at load time:
//!
//! ```text
//! <adi_dir>/lib/<android-abi>/libstoreservicescore.so
//! <adi_dir>/lib/<android-abi>/libCoreADI.so
//! ```
//!
//! `omnisette`'s `StoreServicesCoreADIProxy` reads exactly this path, picking the
//! `<android-abi>` that matches the host CPU (e.g. `x86_64` on a desktop Linux
//! box). The libraries are then loaded and run via Dadoum's `android-loader` — see
//! [`crate::signing::adi`].

use crate::error::{AppError, Result};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The two ADI libraries extracted from the Apple Music APK.
pub const ADI_LIB_NAMES: [&str; 2] = ["libstoreservicescore.so", "libCoreADI.so"];

/// The Android ABI directory name (inside the APK's `lib/`, and in our on-disk
/// layout) matching the host CPU. Mirrors the `cfg(target_arch)` mapping inside
/// `omnisette`'s `store_services_core` so the extracted path is the one it reads.
pub const fn host_apk_abi() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        "x86_64"
    }
    #[cfg(target_arch = "aarch64")]
    {
        "arm64-v8a"
    }
    #[cfg(target_arch = "arm")]
    {
        "armeabi-v7a"
    }
    #[cfg(target_arch = "x86")]
    {
        "x86"
    }
}

/// The directory the ADI `.so` files live in for this host: `<adi_dir>/lib/<abi>`.
pub fn lib_dir(adi_dir: &Path) -> PathBuf {
    adi_dir.join("lib").join(host_apk_abi())
}

/// Whether both ADI libraries are already present for this host. This is the
/// cheap check the setup flow uses to decide if provisioning is still required.
pub fn libs_present(adi_dir: &Path) -> bool {
    let dir = lib_dir(adi_dir);
    ADI_LIB_NAMES.iter().all(|name| dir.join(name).is_file())
}

/// Extract the two ADI libraries from an Apple Music `apk` into `adi_dir`,
/// placing them where `omnisette` will look. Idempotent: re-running overwrites.
///
/// Returns [`AppError::AnisetteAdiIncompatible`] if the APK does not contain
/// native libraries for this host's ABI (e.g. an arm-only APK on an x86_64 box) —
/// the user needs an APK build that includes `lib/<abi>/`.
pub fn provision_from_apk(apk: &Path, adi_dir: &Path) -> Result<()> {
    let abi = host_apk_abi();
    let file = std::fs::File::open(apk).map_err(AppError::Io)?;
    let mut archive = zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| AppError::Internal(format!("not a valid APK (zip) archive: {e}")))?;

    let dest = lib_dir(adi_dir);
    std::fs::create_dir_all(&dest).map_err(AppError::Io)?;

    for name in ADI_LIB_NAMES {
        let entry_path = format!("lib/{abi}/{name}");
        let mut entry = archive.by_name(&entry_path).map_err(|_| {
            tracing::warn!(
                target: "reside::adi",
                entry = %entry_path,
                "Apple Music APK has no {abi} native libraries"
            );
            AppError::AnisetteAdiIncompatible
        })?;
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes).map_err(AppError::Io)?;

        let out_path = dest.join(name);
        std::fs::write(&out_path, &bytes).map_err(AppError::Io)?;
        // Loadable native code must be executable.
        std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(0o755))
            .map_err(AppError::Io)?;
        tracing::info!(target: "reside::adi", lib = name, bytes = bytes.len(), "extracted ADI library");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a minimal fake "APK" (a zip) carrying the two ADI libs under
    /// `lib/<abi>/`, plus unrelated entries, to exercise extraction.
    fn fake_apk(path: &Path, abi: &str, include_libs: bool) {
        let f = std::fs::File::create(path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        zw.start_file("AndroidManifest.xml", opts).unwrap();
        zw.write_all(b"<manifest/>").unwrap();
        if include_libs {
            for name in ADI_LIB_NAMES {
                zw.start_file(format!("lib/{abi}/{name}"), opts).unwrap();
                zw.write_all(format!("fake-{name}").as_bytes()).unwrap();
            }
        }
        zw.finish().unwrap();
    }

    #[test]
    fn extracts_libs_into_omnisette_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let apk = tmp.path().join("AppleMusic.apk");
        let adi = tmp.path().join("adi");
        fake_apk(&apk, host_apk_abi(), true);

        assert!(!libs_present(&adi));
        provision_from_apk(&apk, &adi).unwrap();
        assert!(libs_present(&adi));

        for name in ADI_LIB_NAMES {
            let p = lib_dir(&adi).join(name);
            assert!(p.is_file());
            assert_eq!(p.metadata().unwrap().permissions().mode() & 0o111, 0o111);
            assert_eq!(
                std::fs::read(&p).unwrap(),
                format!("fake-{name}").into_bytes()
            );
        }
    }

    #[test]
    fn apk_without_host_abi_is_incompatible() {
        let tmp = tempfile::tempdir().unwrap();
        let apk = tmp.path().join("AppleMusic.apk");
        let adi = tmp.path().join("adi");
        // Ship libs for a bogus ABI that never matches the host.
        fake_apk(&apk, "mips-imaginary", true);

        let err = provision_from_apk(&apk, &adi).unwrap_err();
        assert!(matches!(err, AppError::AnisetteAdiIncompatible));
        assert!(!libs_present(&adi));
    }

    #[test]
    fn rejects_non_zip_input() {
        let tmp = tempfile::tempdir().unwrap();
        let apk = tmp.path().join("not-an-apk.bin");
        std::fs::write(&apk, b"this is not a zip").unwrap();
        let adi = tmp.path().join("adi");
        assert!(provision_from_apk(&apk, &adi).is_err());
    }
}
