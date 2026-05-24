//! Driver for the forked Sideloader CLI — ReSide's "proven signer".
//!
//! Rather than reimplement Apple's fragile developer-services protocol, ReSide
//! shells out to a forked Dadoum Sideloader CLI (GPL-3.0) that already does it
//! well, and adds the things Sideloader lacks: stored credentials, Wi-Fi install,
//! and unattended auto-refresh.
//!
//! The fork accepts credentials over stdin when `RESIDE_NONINTERACTIVE=1` is set
//! (never on the command line, so they can't leak via `/proc`). If Apple demands
//! 2FA mid-run it prints the marker [`TWO_FA_MARKER`] and exits [`EXIT_2FA`]; a
//! trusted device skips 2FA entirely, which is what makes unattended refresh work.
//!
//! This module owns: credential storage (via [`SecureStore`]), locating the
//! binary, spawning it with creds piped in, and classifying the outcome into the
//! ReSide error taxonomy.

use crate::error::{AppError, Result, Secret};
use crate::secure_storage::SecureStore;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Keyring keys for the (single, for now) Apple account.
const KEY_APPLE_ID: &str = "reside.apple_id";
const KEY_APPLE_PASSWORD: &str = "reside.apple_password";

/// Env var that puts the forked Sideloader CLI into non-interactive mode.
const ENV_NONINTERACTIVE: &str = "RESIDE_NONINTERACTIVE";
/// Env var overriding the Sideloader binary location (else `sideloader` on PATH).
const ENV_SIDELOADER_BIN: &str = "RESIDE_SIDELOADER_BIN";

/// Marker the fork prints to stderr when Apple requires 2FA during an
/// automated run (see the fork's `cli_frontend.d`).
const TWO_FA_MARKER: &str = "RESIDE_2FA_REQUIRED";
/// Exit code the fork uses for the same condition.
const EXIT_2FA: i32 = 2;

/// An Apple ID and password. The password is wrapped in [`Secret`] so it is
/// redacted in logs and debug output; only [`Secret::expose`] reveals it, and
/// only when piping to the signer's stdin.
#[derive(Clone)]
pub struct AppleCredentials {
    pub apple_id: String,
    pub password: Secret<String>,
}

impl std::fmt::Debug for AppleCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppleCredentials")
            .field("apple_id", &self.apple_id)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Persist the Apple account credentials in the secret store.
pub fn store_credentials(store: &SecureStore, creds: &AppleCredentials) -> Result<()> {
    store.set(KEY_APPLE_ID, &creds.apple_id)?;
    store.set(KEY_APPLE_PASSWORD, creds.password.expose())?;
    Ok(())
}

/// Load stored credentials, or `None` if the user hasn't signed in yet.
pub fn load_credentials(store: &SecureStore) -> Result<Option<AppleCredentials>> {
    let (Some(apple_id), Some(password)) =
        (store.get(KEY_APPLE_ID)?, store.get(KEY_APPLE_PASSWORD)?)
    else {
        return Ok(None);
    };
    Ok(Some(AppleCredentials {
        apple_id,
        password: Secret::new(password),
    }))
}

/// Forget the stored credentials (sign-out).
pub fn clear_credentials(store: &SecureStore) -> Result<()> {
    store.delete(KEY_APPLE_ID)?;
    store.delete(KEY_APPLE_PASSWORD)?;
    Ok(())
}

/// Resolve the Sideloader binary: `RESIDE_SIDELOADER_BIN` if set, else the
/// `sideloader` on `PATH`.
pub fn sideloader_binary() -> PathBuf {
    std::env::var_os(ENV_SIDELOADER_BIN)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("sideloader"))
}

/// Raw result of a finished Sideloader invocation. `output` is stdout and
/// stderr concatenated: Sideloader logs through slf4d, which writes to stdout,
/// so we must inspect both streams to classify a failure.
struct RawOutcome {
    code: Option<i32>,
    output: String,
}

/// Classify a finished Sideloader run into the ReSide taxonomy.
///
/// Pure so it can be unit-tested without spawning a process:
/// - exit 0 → `Ok(())`
/// - the 2FA marker or [`EXIT_2FA`] → [`AppError::AppleAuth2faRequired`]
/// - a login failure → [`AppError::AppleAuthCredentialsInvalid`]
/// - no device attached → [`AppError::DeviceOffline`]
/// - a free-account weekly limit → the matching quota error
/// - anything else → [`AppError::Internal`]
fn classify(outcome: &RawOutcome) -> Result<()> {
    if outcome.code == Some(0) {
        return Ok(());
    }
    if outcome.code == Some(EXIT_2FA) || outcome.output.contains(TWO_FA_MARKER) {
        return Err(AppError::AppleAuth2faRequired);
    }
    let s = outcome.output.to_lowercase();
    if s.contains("can't log-in") || s.contains("cant log-in") || s.contains("log-in") {
        return Err(AppError::AppleAuthCredentialsInvalid);
    }
    // `sideloader install` prints this when no (matching) device is attached.
    if s.contains("no device connected") || s.contains("device connected") {
        return Err(AppError::DeviceOffline);
    }
    // The delegated signer talks to the device through libimobiledevice, whose
    // pairing is separate from ReSide's own. If that system pairing is missing
    // or stale the device rejects it with INVALID_HOST_ID — the user needs to
    // (re-)trust this computer (e.g. `idevicepair pair`).
    if s.contains("invalid_host_id") || s.contains("not paired") {
        return Err(AppError::DeviceNotTrusted);
    }
    // Free Apple-ID weekly quotas (10 App IDs / 10 device registrations).
    if s.contains("maximum app id") || s.contains("maximum number of app id") {
        return Err(AppError::AppleAppIdLimitReached);
    }
    if s.contains("maximum") && s.contains("device") {
        return Err(AppError::AppleDevDeviceRegLimitReached);
    }
    Err(AppError::Internal(format!(
        "sideloader exited with {:?}",
        outcome.code
    )))
}

/// Run a Sideloader subcommand non-interactively, feeding credentials on stdin.
/// `extra_stdin_lines` are appended after the Apple ID + password (e.g. a 2FA
/// code when re-invoking after [`AppError::AppleAuth2faRequired`]).
async fn run(
    creds: &AppleCredentials,
    args: &[&str],
    extra_stdin_lines: &[&str],
) -> Result<RawOutcome> {
    let bin = sideloader_binary();
    let mut child = Command::new(&bin)
        .args(args)
        .env(ENV_NONINTERACTIVE, "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            AppError::Internal(format!(
                "could not launch the Sideloader signer ({}): {e}",
                bin.display()
            ))
        })?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::Internal("signer stdin unavailable".into()))?;
        let mut payload = format!("{}\n{}\n", creds.apple_id, creds.password.expose());
        for line in extra_stdin_lines {
            payload.push_str(line);
            payload.push('\n');
        }
        stdin.write_all(payload.as_bytes()).await.map_err(|e| {
            AppError::Internal(format!("failed writing credentials to signer: {e}"))
        })?;
        // Drop closes stdin so the child stops waiting for more input.
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| AppError::Internal(format!("signer did not complete: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let code = output.status.code();

    // On any non-zero exit, log the tool's own output so a failed sign/install
    // is diagnosable from the terminal (it never reaches the redacted UI error).
    // Sideloader does not echo the password; the Apple ID may appear, which is
    // acceptable in a local diagnostic log.
    if code != Some(0) {
        tracing::warn!(
            ?code,
            args = ?args,
            stdout = %stdout.trim(),
            stderr = %stderr.trim(),
            "sideloader exited non-zero"
        );
    }

    Ok(RawOutcome {
        code,
        output: format!("{stdout}\n{stderr}"),
    })
}

/// Verify the stored/supplied credentials authenticate with Apple by listing the
/// account's teams. Returns `Ok(())` on success, [`AppError::AppleAuth2faRequired`]
/// if Apple wants a 2FA code, or [`AppError::AppleAuthCredentialsInvalid`].
///
/// This is the Rust-side equivalent of the `sideloader team list` check that was
/// validated manually, and the smoke test that the bridge works end-to-end.
pub async fn verify_login(creds: &AppleCredentials) -> Result<()> {
    let outcome = run(creds, &["team", "list"], &[]).await?;
    classify(&outcome)
}

/// Inputs for one sign-and-install run. Borrowed so the caller keeps ownership
/// of the credentials and paths.
pub struct InstallRequest<'a> {
    pub creds: &'a AppleCredentials,
    /// Path to the (unsigned) `.ipa`. Sideloader signs it in place during install.
    pub ipa_path: &'a Path,
    /// Target device UDID. Required even with one device so the fork doesn't
    /// have to guess.
    pub udid: &'a str,
    /// A 2FA code to satisfy a prior [`AppError::AppleAuth2faRequired`]. On the
    /// first attempt this is `None`; the fork only asks when the device isn't
    /// already trusted.
    pub two_fa_code: Option<&'a str>,
}

/// Sign and install an IPA by delegating to the forked `sideloader install`,
/// which renames the app, registers its identifier, signs it, and installs it
/// over USB in one pass.
///
/// `--singlethread` is deliberate: the user's recurring "signing gets stuck"
/// pain is the multithreaded signer being flaky, and Sideloader documents
/// single-threaded mode as trading speed for consistency. We take the slower,
/// reliable path here.
///
/// Returns [`AppError::AppleAuth2faRequired`] if Apple demanded a code and none
/// was supplied — re-call with [`InstallRequest::two_fa_code`] set.
pub async fn install(req: &InstallRequest<'_>) -> Result<()> {
    let ipa = req.ipa_path.to_string_lossy();
    let args = [
        "install",
        ipa.as_ref(),
        "--udid",
        req.udid,
        "--singlethread",
    ];
    let extra: Vec<&str> = req.two_fa_code.into_iter().collect();
    let outcome = run(req.creds, &args, &extra).await?;
    classify(&outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_round_trip_and_redact() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SecureStore::File(tmp.path().join("secrets"));

        assert!(load_credentials(&store).unwrap().is_none());

        let creds = AppleCredentials {
            apple_id: "user@example.com".into(),
            password: Secret::new("hunter2".into()),
        };
        store_credentials(&store, &creds).unwrap();

        let loaded = load_credentials(&store).unwrap().unwrap();
        assert_eq!(loaded.apple_id, "user@example.com");
        assert_eq!(loaded.password.expose(), "hunter2");
        // The password must never appear in Debug output.
        assert!(!format!("{loaded:?}").contains("hunter2"));

        clear_credentials(&store).unwrap();
        assert!(load_credentials(&store).unwrap().is_none());
    }

    #[test]
    fn binary_path_honors_env_override() {
        std::env::set_var(ENV_SIDELOADER_BIN, "/opt/custom/sideloader");
        assert_eq!(sideloader_binary(), PathBuf::from("/opt/custom/sideloader"));
        std::env::remove_var(ENV_SIDELOADER_BIN);
        assert_eq!(sideloader_binary(), PathBuf::from("sideloader"));
    }

    /// Build an outcome whose recognizable text lands on *stdout* — where
    /// Sideloader's slf4d logger actually writes it. This guards the regression
    /// where classification only looked at stderr.
    fn on_stdout(code: i32, stdout: &str) -> RawOutcome {
        RawOutcome {
            code: Some(code),
            output: format!("{stdout}\n"), // run() puts stdout first, stderr after
        }
    }

    #[test]
    fn classify_maps_outcomes_to_taxonomy() {
        assert!(classify(&RawOutcome {
            code: Some(0),
            output: String::new()
        })
        .is_ok());

        assert!(matches!(
            classify(&RawOutcome {
                code: Some(EXIT_2FA),
                output: String::new()
            }),
            Err(AppError::AppleAuth2faRequired)
        ));

        // The 2FA marker / login error / device error all arrive on stdout.
        assert!(matches!(
            classify(&on_stdout(2, "RESIDE_2FA_REQUIRED")),
            Err(AppError::AppleAuth2faRequired)
        ));

        assert!(matches!(
            classify(&on_stdout(1, "ERROR Can't log-in! ...")),
            Err(AppError::AppleAuthCredentialsInvalid)
        ));

        assert!(matches!(
            classify(&on_stdout(1, "ERROR No device connected.")),
            Err(AppError::DeviceOffline)
        ));

        assert!(matches!(
            classify(&on_stdout(
                1,
                "iMobileDeviceException ... error LOCKDOWN_E_INVALID_HOST_ID"
            )),
            Err(AppError::DeviceNotTrusted)
        ));

        assert!(matches!(
            classify(&on_stdout(1, "Maximum App IDs reached for this account")),
            Err(AppError::AppleAppIdLimitReached)
        ));

        assert!(matches!(
            classify(&on_stdout(1, "some other failure")),
            Err(AppError::Internal(_))
        ));
    }
}
