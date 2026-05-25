//! Background-agent *autopilot*: installs and removes the OS-level trigger that
//! runs the refresh engine ([`super::scheduler::refresh_due`]) on a schedule,
//! so apps keep refreshing even while ReSide is closed. Task 11c, slice 2.
//!
//! ## What this module does (and doesn't)
//! It does *not* refresh anything itself — that's [`super::scheduler`], which is
//! deliberately trigger-agnostic. This module only writes/removes the trigger:
//! a **systemd user timer** where available, or an **XDG autostart** entry as a
//! fallback on hosts without a systemd user instance. The headless `reside-agent`
//! binary is what those triggers actually launch; it calls `refresh_due`.
//!
//! ## Two run modes the agent binary must support
//! - [`AgentMode::Run`] — one-shot: sweep once and exit. The systemd *timer*
//!   fires this every few hours; systemd owns the cadence.
//! - [`AgentMode::Loop`] — sleep-loop: sweep, sleep ~6h, repeat. The XDG
//!   autostart fallback uses this, because autostart only fires once at login
//!   and there's no timer to re-trigger it.
//!
//! ## Yielding to the UI
//! Nothing special happens here for coordination — `refresh_due` already takes
//! the single-writer [`crate::proc_lock`]. If the UI is mid-install/refresh the
//! agent's sweep simply finds the lock held and returns without running, then
//! tries again on the next tick. "Yield" means *never collide*, not *never run
//! while the app is open*.
//!
//! ## Scope: USB-only for now
//! 11c refreshes over the USB sideloader path, so the units intentionally do
//! **not** depend on the Wi-Fi `reside-tunneld` service (that's 11d). "Unattended"
//! here means "keep the iPhone plugged in."

use crate::error::{AppError, Result};
use crate::paths::Paths;
use std::path::{Path, PathBuf};
use std::process::Command;

/// systemd unit / autostart basenames. Kept in one place so install, uninstall,
/// and status all agree.
const SERVICE_UNIT: &str = "reside-agent.service";
const TIMER_UNIT: &str = "reside-agent.timer";
const AUTOSTART_DESKTOP: &str = "reside-agent.desktop";

/// Default sweep cadence. 6h against the engine's 2-day refresh lead window
/// leaves plenty of slop; the cadence is systemd's job, the lead window is the
/// engine's — see [`super::scheduler::REFRESH_LEAD_SECS`].
pub const DEFAULT_INTERVAL_HOURS: u32 = 6;

/// How the background agent is wired into the OS on this host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMechanism {
    /// A `reside-agent.timer` user unit (preferred): survives logout, catches up
    /// after the machine was asleep/off via `Persistent=true`.
    Systemd,
    /// An `~/.config/autostart` entry that starts the agent's sleep-loop at
    /// login. Fallback for hosts without a systemd user instance.
    XdgAutostart,
}

/// The run mode the launched `reside-agent` binary should use. Mirrors the CLI
/// arg the units pass (`run` vs `loop`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    /// Sweep once and exit (driven by the systemd timer).
    Run,
    /// Sweep, sleep, repeat (driven by the autostart fallback).
    Loop,
}

impl AgentMode {
    /// The CLI token the units write into `ExecStart`, and that the agent binary
    /// parses back out of `argv`.
    pub fn as_arg(self) -> &'static str {
        match self {
            AgentMode::Run => "run",
            AgentMode::Loop => "loop",
        }
    }

    /// Parse the agent binary's first CLI argument; defaults to [`AgentMode::Run`]
    /// when absent or unrecognized (the safe one-shot behavior).
    pub fn from_arg(arg: Option<&str>) -> Self {
        match arg {
            Some("loop") => AgentMode::Loop,
            _ => AgentMode::Run,
        }
    }
}

/// Everything the unit/desktop generators need to point at the right binary
/// with the right environment.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Absolute path to the `reside-agent` binary the trigger should launch.
    pub exec_path: PathBuf,
    /// The Sideloader binary path to bake into the unit's environment. systemd
    /// user services start with a bare environment, so unless we pass this the
    /// agent would fall back to bare `sideloader` on `PATH` (see
    /// [`crate::signer::sideloader_binary`]). `None` => rely on `PATH`.
    pub sideloader_bin: Option<PathBuf>,
    /// Sweep cadence in hours (systemd timer / autostart-loop sleep).
    pub interval_hours: u32,
}

impl AgentConfig {
    /// A config pointing at `exec_path` with the default cadence and the given
    /// (optional) Sideloader path.
    pub fn new(exec_path: impl Into<PathBuf>, sideloader_bin: Option<PathBuf>) -> Self {
        Self {
            exec_path: exec_path.into(),
            sideloader_bin,
            interval_hours: DEFAULT_INTERVAL_HOURS,
        }
    }
}

/// Current state of the autopilot, surfaced to the UI toggle.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    /// Whether the trigger is currently installed/enabled.
    pub enabled: bool,
    /// Which mechanism is (or would be) used on this host. Present even when
    /// disabled so the UI can explain what enabling will do.
    pub mechanism: AgentMechanism,
    /// One-line plain-English description for the UI.
    pub detail: String,
}

// --- mechanism detection -----------------------------------------------------

/// Pick the trigger mechanism for this host. We use systemd's *user* instance
/// when one is actually running — detected by the presence of
/// `$XDG_RUNTIME_DIR/systemd` — and fall back to XDG autostart otherwise. This
/// is the same signal `systemctl --user` relies on, without spawning a process.
pub fn detect_mechanism() -> AgentMechanism {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) if Path::new(&dir).join("systemd").is_dir() => AgentMechanism::Systemd,
        _ => AgentMechanism::XdgAutostart,
    }
}

// --- pure generators (unit-tested) -------------------------------------------

/// The `reside-agent.service` text: a `oneshot` that runs the agent once.
/// Deliberately has no `[Install]` (the timer pulls it) and **no dependency on
/// `reside-tunneld`** — 11c is USB-only.
pub fn service_unit(cfg: &AgentConfig) -> String {
    let mut s = String::new();
    s.push_str("[Unit]\n");
    s.push_str("Description=ReSide background app-refresh agent\n");
    s.push_str("Documentation=https://github.com/everlasting-marshall/reside\n");
    s.push('\n');
    s.push_str("[Service]\n");
    s.push_str("Type=oneshot\n");
    if let Some(bin) = &cfg.sideloader_bin {
        // The agent resolves the signer from RESIDE_SIDELOADER_BIN; bake the
        // path in because user services don't inherit the desktop environment.
        s.push_str(&format!(
            "Environment=RESIDE_SIDELOADER_BIN={}\n",
            bin.display()
        ));
    }
    s.push_str(&format!(
        "ExecStart={} {}\n",
        cfg.exec_path.display(),
        AgentMode::Run.as_arg()
    ));
    s
}

/// The `reside-agent.timer` text: fire every `interval_hours`, and `Persistent`
/// so a missed window (laptop asleep/off) runs on the next boot rather than
/// silently letting an app expire.
pub fn timer_unit(cfg: &AgentConfig) -> String {
    let step = cfg.interval_hours.clamp(1, 24);
    let mut s = String::new();
    s.push_str("[Unit]\n");
    s.push_str("Description=ReSide background app-refresh schedule\n");
    s.push('\n');
    s.push_str("[Timer]\n");
    // e.g. "00/6" => 00:00, 06:00, 12:00, 18:00.
    s.push_str(&format!("OnCalendar=*-*-* 00/{step}:00:00\n"));
    s.push_str("Persistent=true\n");
    // Spread load / avoid every machine hitting Apple on the hour.
    s.push_str("RandomizedDelaySec=900\n");
    s.push('\n');
    s.push_str("[Install]\n");
    s.push_str("WantedBy=timers.target\n");
    s
}

/// The XDG autostart `.desktop` fallback. Launches the agent's *loop* mode at
/// login (autostart fires once, so the agent must self-schedule). Sets the
/// Sideloader env inline via `env` since `.desktop` has no environment field.
pub fn autostart_desktop(cfg: &AgentConfig) -> String {
    let exec = match &cfg.sideloader_bin {
        Some(bin) => format!(
            "env RESIDE_SIDELOADER_BIN={} {} {}",
            bin.display(),
            cfg.exec_path.display(),
            AgentMode::Loop.as_arg()
        ),
        None => format!("{} {}", cfg.exec_path.display(), AgentMode::Loop.as_arg()),
    };
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=ReSide Auto-Refresh\n\
         Comment=Keeps sideloaded apps refreshed in the background\n\
         Exec={exec}\n\
         X-GNOME-Autostart-enabled=true\n\
         NoDisplay=true\n"
    )
}

// --- install / uninstall / status (IO) ---------------------------------------

/// Install the background trigger using whichever mechanism this host supports,
/// returning the resulting status. Idempotent: re-installing overwrites the unit
/// files and re-enables the timer.
pub fn install(paths: &Paths, cfg: &AgentConfig) -> Result<AgentStatus> {
    match detect_mechanism() {
        AgentMechanism::Systemd => install_systemd(paths, cfg),
        AgentMechanism::XdgAutostart => install_autostart(paths, cfg),
    }
}

/// Remove the background trigger by *both* mechanisms, so toggling off cleans up
/// regardless of which one installed it (e.g. if the host changed). Idempotent.
pub fn uninstall(paths: &Paths) -> Result<AgentStatus> {
    uninstall_systemd(paths)?;
    uninstall_autostart(paths)?;
    status(paths)
}

/// Report whether the trigger is currently enabled and how it's wired.
pub fn status(paths: &Paths) -> Result<AgentStatus> {
    let mechanism = detect_mechanism();
    let enabled = match mechanism {
        AgentMechanism::Systemd => timer_is_enabled(),
        AgentMechanism::XdgAutostart => paths.autostart_dir().join(AUTOSTART_DESKTOP).exists(),
    };
    Ok(AgentStatus {
        enabled,
        mechanism,
        detail: detail_for(mechanism, enabled),
    })
}

fn detail_for(mechanism: AgentMechanism, enabled: bool) -> String {
    match (mechanism, enabled) {
        (_, false) => "ReSide only refreshes apps while it's open.".into(),
        (AgentMechanism::Systemd, true) => {
            "On — checks every 6 hours, even when ReSide is closed. Keep your iPhone plugged in."
                .into()
        }
        (AgentMechanism::XdgAutostart, true) => {
            "On — runs at login (your system has no timed-task service). Keep your iPhone plugged in."
                .into()
        }
    }
}

fn install_systemd(paths: &Paths, cfg: &AgentConfig) -> Result<AgentStatus> {
    let dir = paths.systemd_user_dir();
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(SERVICE_UNIT), service_unit(cfg))?;
    std::fs::write(dir.join(TIMER_UNIT), timer_unit(cfg))?;

    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", "--now", TIMER_UNIT])?;

    tracing::info!(dir = %dir.display(), "installed systemd user timer for refresh agent");
    status(paths)
}

fn uninstall_systemd(paths: &Paths) -> Result<()> {
    let dir = paths.systemd_user_dir();
    let timer = dir.join(TIMER_UNIT);
    // Only touch systemctl if the unit was actually installed, so an uninstall
    // on a host that never had it stays quiet.
    if timer.exists() {
        // Best-effort: a disabled/absent timer must not fail the uninstall.
        let _ = systemctl(&["disable", "--now", TIMER_UNIT]);
    }
    let _ = std::fs::remove_file(&timer);
    let _ = std::fs::remove_file(dir.join(SERVICE_UNIT));
    if timer.exists() {
        return Err(AppError::Internal(format!(
            "could not remove {}",
            timer.display()
        )));
    }
    let _ = systemctl(&["daemon-reload"]);
    Ok(())
}

fn install_autostart(paths: &Paths, cfg: &AgentConfig) -> Result<AgentStatus> {
    let dir = paths.autostart_dir();
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(AUTOSTART_DESKTOP), autostart_desktop(cfg))?;
    tracing::info!(dir = %dir.display(), "installed XDG autostart entry for refresh agent");
    status(paths)
}

fn uninstall_autostart(paths: &Paths) -> Result<()> {
    let _ = std::fs::remove_file(paths.autostart_dir().join(AUTOSTART_DESKTOP));
    Ok(())
}

/// Run `systemctl --user <args>`, turning a non-zero exit into an `AppError` so
/// the UI can surface why enabling the autopilot failed.
fn systemctl(args: &[&str]) -> Result<()> {
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .map_err(|e| AppError::Internal(format!("could not run systemctl --user: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Internal(format!(
            "systemctl --user {} failed: {}",
            args.join(" "),
            stderr.trim()
        )));
    }
    Ok(())
}

/// Whether `reside-agent.timer` is enabled. `is-enabled` exits non-zero (without
/// erroring) when the unit is disabled or absent, so we read the printed word
/// rather than the exit status.
fn timer_is_enabled() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-enabled", TIMER_UNIT])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "enabled")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> AgentConfig {
        AgentConfig::new("/usr/bin/reside-agent", Some("/opt/sideloader".into()))
    }

    #[test]
    fn mode_arg_round_trips() {
        assert_eq!(AgentMode::from_arg(Some("run")), AgentMode::Run);
        assert_eq!(AgentMode::from_arg(Some("loop")), AgentMode::Loop);
        // Unknown / missing defaults to the safe one-shot.
        assert_eq!(AgentMode::from_arg(None), AgentMode::Run);
        assert_eq!(AgentMode::from_arg(Some("nonsense")), AgentMode::Run);
        assert_eq!(AgentMode::Loop.as_arg(), "loop");
    }

    #[test]
    fn service_unit_runs_agent_once_with_signer_env_and_no_tunnel_dep() {
        let s = service_unit(&cfg());
        assert!(s.contains("Type=oneshot"));
        assert!(s.contains("ExecStart=/usr/bin/reside-agent run"));
        assert!(s.contains("Environment=RESIDE_SIDELOADER_BIN=/opt/sideloader"));
        // 11c is USB-only: the unit must not chain the Wi-Fi tunnel service.
        assert!(
            !s.contains("reside-tunneld"),
            "agent must not depend on the Wi-Fi tunnel service in 11c"
        );
        // No [Install]: the timer activates the service, not a target want.
        assert!(!s.contains("[Install]"));
    }

    #[test]
    fn service_unit_omits_env_when_no_signer_path() {
        let s = service_unit(&AgentConfig::new("/usr/bin/reside-agent", None));
        assert!(!s.contains("RESIDE_SIDELOADER_BIN"));
        assert!(s.contains("ExecStart=/usr/bin/reside-agent run"));
    }

    #[test]
    fn timer_fires_periodically_and_catches_up() {
        let s = timer_unit(&cfg());
        assert!(s.contains("OnCalendar=*-*-* 00/6:00:00"));
        // Persistent is what saves a laptop that was off across the expiry.
        assert!(s.contains("Persistent=true"));
        assert!(s.contains("WantedBy=timers.target"));
    }

    #[test]
    fn timer_interval_is_clamped_to_a_sane_range() {
        let mut c = cfg();
        c.interval_hours = 0;
        assert!(timer_unit(&c).contains("00/1:00:00"));
        c.interval_hours = 999;
        assert!(timer_unit(&c).contains("00/24:00:00"));
    }

    #[test]
    fn autostart_uses_loop_mode_with_inline_env() {
        let d = autostart_desktop(&cfg());
        assert!(d.contains("Type=Application"));
        assert!(d.contains("env RESIDE_SIDELOADER_BIN=/opt/sideloader /usr/bin/reside-agent loop"));
        assert!(d.contains("X-GNOME-Autostart-enabled=true"));
    }

    #[test]
    fn detail_text_reflects_state() {
        assert!(detail_for(AgentMechanism::Systemd, false).contains("only refreshes"));
        assert!(detail_for(AgentMechanism::Systemd, true).contains("every 6 hours"));
        assert!(detail_for(AgentMechanism::XdgAutostart, true).contains("at login"));
    }

    #[test]
    fn autostart_install_uninstall_roundtrip() {
        // The autostart path is pure filesystem — exercise it end to end under a
        // sandboxed Paths root (no systemctl involved).
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::with_root(tmp.path());
        let desktop = paths.autostart_dir().join(AUTOSTART_DESKTOP);

        install_autostart(&paths, &cfg()).unwrap();
        assert!(desktop.exists(), "autostart entry should be written");

        uninstall_autostart(&paths).unwrap();
        assert!(!desktop.exists(), "autostart entry should be removed");
        // Removing again is fine (idempotent).
        uninstall_autostart(&paths).unwrap();
    }
}
