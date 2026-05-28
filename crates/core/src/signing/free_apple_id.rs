//! Free Apple ID credential flow.
//!
//! ⚠️ **PARKED** — the abandoned native-signing path. The live app drives the
//! forked Sideloader CLI for the full free-Apple-ID flow (auth + signing). See
//! [`super`] (signing/mod.rs) and [`crate::signer`].
//!
//! **Scope note (2026-05-23): AUTHENTICATION half only.** The plan's full flow is
//! steps 0–8 (anisette → GSA login → 2FA → CSR → cert → device register → App ID
//! → provisioning profile → sign). Steps 3–7 — the Apple Developer Services half
//! that yields a device-acceptable signing identity — are **not implemented
//! upstream**: `apple-dev-apis`'s `XcodeSession::with` is a `todo!()` stub and no
//! `developerservices2` client exists anywhere in `apple-private-apis`. Writing
//! that layer is the project's biggest open decision (plan.md §Known Foot-Guns:
//! "do not silently rewrite Apple auth in-app — open an issue and pause").
//!
//! So this module currently covers steps 0–2: provision-gate the local ADI
//! libraries, then sign in with Apple ID + password + 2FA via `icloud_auth`. This
//! is the checkpoint that proves the (unmaintained, 2024-era) auth stack still
//! authenticates against Apple's current GSA endpoints *before* we invest in the
//! Developer Services build. [`AuthenticatedAccount`] holds the result; turning it
//! into a [`SigningProvider`](super::SigningProvider) is the deferred Half 2.
//!
//! **No remote anisette.** [`authenticate`] gates on [`adi::require_provisioned`]
//! and [`adi::verify_local_provider`] first, so `icloud_auth`'s silent
//! remote-anisette fallback can never engage (see [`adi`] for why that matters).

use crate::error::{AppError, Result};
use crate::signing::adi;
use icloud_auth::{AppleAccount, Error as IcloudError};
use std::path::Path;

/// A logged-in Apple account. Half 2 (CSR → cert → device → App ID → profile)
/// will consume this to produce a [`SigningProvider`](super::SigningProvider);
/// for now it just confirms authentication succeeded.
pub struct AuthenticatedAccount {
    account: AppleAccount,
}

impl AuthenticatedAccount {
    /// Whether Apple returned a usable password-equivalent token (PET) — the
    /// signal that login fully completed (including any 2FA).
    pub fn has_pet(&self) -> bool {
        self.account.get_pet().is_some()
    }

    /// The user's name as reported by Apple `(first, last)`, for UI confirmation.
    pub fn name(&self) -> (String, String) {
        self.account.get_name()
    }

    /// Access the underlying account (for the future Developer Services half).
    pub fn account(&self) -> &AppleAccount {
        &self.account
    }
}

/// Translate `icloud_auth` failures into ReSide's error taxonomy. The upstream
/// error messages are not user-facing; the category drives remediation text.
fn map_auth_error(e: IcloudError) -> AppError {
    match e {
        // SRP rejected the credentials. Apple folds rate-limiting / account lock
        // into the same SRP failure with a distinguishing code/message.
        IcloudError::AuthSrp => AppError::AppleAuthCredentialsInvalid,
        IcloudError::AuthSrpWithMessage(code, msg) => {
            let m = msg.to_lowercase();
            if code == -21669
                || m.contains("too many")
                || m.contains("locked")
                || m.contains("rate")
            {
                AppError::AppleAuthRateLimited
            } else {
                AppError::AppleAuthCredentialsInvalid
            }
        }
        // Wrong 2FA code — recoverable; the UI re-prompts.
        IcloudError::Bad2faCode => AppError::AppleAuth2faRequired,
        // An interactive step we don't drive (e.g. appleid.apple.com repair).
        IcloudError::ExtraStep(_) => AppError::AppleAuthProtocolChanged,
        // Unexpected response shape → Apple likely changed the flow.
        IcloudError::Parse | IcloudError::PlistError(_) => AppError::AppleAuthProtocolChanged,
        IcloudError::ErrorGettingAnisette(_) => AppError::AnisetteGenFailed,
        IcloudError::ReqwestError(e) => {
            tracing::warn!(target: "reside::auth", error = %e, "Apple auth transport error");
            AppError::Internal("Apple authentication request failed".into())
        }
    }
}

/// Sign in to Apple with `apple_id` + `password`, driving 2FA via the
/// `request_2fa_code` callback (called when Apple prompts; return the 6-digit
/// trusted-device code). Local anisette must already be provisioned — otherwise
/// this fails fast with [`AppError::AnisetteAdiUnavailable`] rather than silently
/// using a remote anisette server.
pub async fn authenticate(
    adi_dir: &Path,
    apple_id: &str,
    password: &str,
    request_2fa_code: impl Fn() -> String,
) -> Result<AuthenticatedAccount> {
    // Gate on LOCAL anisette before any network call (no remote fallback).
    adi::require_provisioned(adi_dir)?;
    adi::verify_local_provider(adi_dir)?;

    let config = adi::anisette_config(adi_dir);
    let id = apple_id.to_string();
    let pw = password.to_string();

    let account = AppleAccount::login(move || (id.clone(), pw.clone()), request_2fa_code, config)
        .await
        .map_err(map_auth_error)?;

    Ok(AuthenticatedAccount { account })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn authenticate_fails_fast_without_adi_libs() {
        // No ADI libraries provisioned → must error before touching the network
        // (and before invoking the credential closures), never falling back to
        // remote anisette.
        let tmp = tempfile::tempdir().unwrap();
        let adi_dir = tmp.path().join("adi");

        let res = authenticate(&adi_dir, "user@example.com", "pw", || {
            panic!("2FA closure must not be reached when ADI is unprovisioned")
        })
        .await;
        assert!(
            matches!(res, Err(AppError::AnisetteAdiUnavailable)),
            "expected AnisetteAdiUnavailable"
        );
    }

    #[test]
    fn error_mapping_covers_the_taxonomy() {
        assert!(matches!(
            map_auth_error(IcloudError::AuthSrp),
            AppError::AppleAuthCredentialsInvalid
        ));
        assert!(matches!(
            map_auth_error(IcloudError::AuthSrpWithMessage(
                -21669,
                "Too many attempts".into()
            )),
            AppError::AppleAuthRateLimited
        ));
        assert!(matches!(
            map_auth_error(IcloudError::AuthSrpWithMessage(
                -20101,
                "bad password".into()
            )),
            AppError::AppleAuthCredentialsInvalid
        ));
        assert!(matches!(
            map_auth_error(IcloudError::Bad2faCode),
            AppError::AppleAuth2faRequired
        ));
        assert!(matches!(
            map_auth_error(IcloudError::ExtraStep("repair".into())),
            AppError::AppleAuthProtocolChanged
        ));
    }

    // ---- Live checkpoints (ignored; require the user's hardware/credentials) ----
    //
    // Step 1 — provision the ADI libraries from an Apple Music APK:
    //   RESIDE_APPLE_MUSIC_APK=/path/to/AppleMusic.apk \
    //   RESIDE_ADI_DIR=/tmp/reside-adi \
    //   cargo test -p reside-core free_apple_id::tests::live_provision_adi -- --ignored --nocapture
    //
    // Step 2 — sign in (uses the same RESIDE_ADI_DIR), typing the 2FA code when asked:
    //   RESIDE_ADI_DIR=/tmp/reside-adi \
    //   RESIDE_APPLE_ID=you@example.com RESIDE_APPLE_PASSWORD='...' \
    //   cargo test -p reside-core free_apple_id::tests::live_login -- --ignored --nocapture

    fn live_adi_dir() -> std::path::PathBuf {
        std::env::var("RESIDE_ADI_DIR")
            .map(std::path::PathBuf::from)
            .expect("set RESIDE_ADI_DIR")
    }

    #[test]
    #[ignore = "requires a real Apple Music APK; run manually"]
    fn live_provision_adi() {
        use crate::setup::adi_provision;
        let apk = std::env::var("RESIDE_APPLE_MUSIC_APK").expect("set RESIDE_APPLE_MUSIC_APK");
        let adi_dir = live_adi_dir();
        adi_provision::provision_from_apk(std::path::Path::new(&apk), &adi_dir).unwrap();
        assert!(adi_provision::libs_present(&adi_dir));
        adi::verify_local_provider(&adi_dir).expect("local ADI provider must load");
        println!("ADI provisioned + loads locally at {}", adi_dir.display());
    }

    #[tokio::test]
    #[ignore = "requires a real Apple ID + 2FA; run manually"]
    async fn live_login() {
        let adi_dir = live_adi_dir();
        let apple_id = std::env::var("RESIDE_APPLE_ID").expect("set RESIDE_APPLE_ID");
        let password = std::env::var("RESIDE_APPLE_PASSWORD").expect("set RESIDE_APPLE_PASSWORD");

        let acct = authenticate(&adi_dir, &apple_id, &password, || {
            use std::io::Write;
            print!("Enter the 6-digit Apple 2FA code: ");
            std::io::stdout().flush().ok();
            let mut line = String::new();
            std::io::stdin().read_line(&mut line).unwrap();
            line.trim().to_string()
        })
        .await
        .expect("login should succeed");

        let (first, last) = acct.name();
        println!(
            "Logged in as {first} {last}; PET present = {}",
            acct.has_pet()
        );
        assert!(acct.has_pet(), "expected a usable PET after login");
    }
}
