# Linux iOS Sideloading App — Architecture Plan

## Context

Linux-first desktop app for signing, sideloading, and auto-refreshing iOS apps on a personal iPhone/iPad — a native Linux alternative to Windows tools like Sideloadly.

**Core value prop:** Sign + sideload an IPA from Linux, then auto-refresh over Wi-Fi before Apple's 7-day free-account signing window expires.

**Primary target:** Arch Linux users via GitHub Releases for v1.x; AUR packaging follows after dogfooding. AppImage/Flatpak are stretch goals.

**Supported iOS scope (v1):** iOS / iPadOS **17.4 or newer only** (RemoteXPC / RSD transport). Pre-17.4 legacy lockdown support is future work — see §iOS Scope.

**Product name:** **ReSide** (binary / package / systemd unit / XDG entry: `reside`).

---

## Tech Stack

| Layer | Choice |
|-------|--------|
| UI framework | Tauri 2 + React + TypeScript |
| Backend | Rust (Cargo workspace: `core` + `tauri-app`) |
| Components | shadcn/ui + Radix + Tailwind CSS |
| Async state | TanStack Query |
| UI state | Zustand |
| Database | SQLite via `sqlx` |
| Secrets | `keyring` crate (Linux Secret Service + filesystem fallback) |
| Device transport | `idevice` crate (RemoteXPC / RSD, USB + Wi-Fi) |
| Background agent | systemd user service (XDG autostart fallback) + tunneld helper |
| Notifications | Tauri notification plugin (requires libnotify-compatible daemon on host) |

---

## Signing Ecosystem

> **⚠️ SUPERSEDED (2026-05-23).** This section describes the original plan to *own* the Apple
> free-signing stack natively in Rust. ReSide has since **pivoted**: the upstream Rust crates
> implement Apple login but not the developer-services (cert/profile) half, so ReSide now drives a
> **fork of Dadoum Sideloader** as the signer and focuses on automation/Wi-Fi/refresh on top. See
> `ONBOARDING.md` and the `project-motivation` memory. The native approach below is parked on the
> `native-signing-path` git branch; read it as historical. (§Known Foot-Guns still applies.)

Free Apple ID signing requires two distinct things:

1. **Apple authentication + provisioning** — authenticating with Apple's GSA (Grand Slam Auth), generating anisette data, registering devices, creating certificates and provisioning profiles. The fragile, potentially-breaking piece.
2. **IPA signing** — unpacking the `.ipa`, patching the embedded `.app`, signing it with a cert + provisioning profile, repacking. Stable and well-understood.

### Key Rust libraries

| Purpose | Crate | Notes |
|---------|-------|-------|
| `.app` bundle signing | `apple-codesign` (indygreg/apple-platform-rs) | Pure Rust, no native deps, runs on Linux. Signs Mach-O + `.app` bundles — IPA wrapping is our responsibility. |
| Anisette generation | `omnisette` (SideStore/apple-private-apis) | Local anisette data; persists synthetic device fingerprint. **Requires Apple's ADI libraries via FFI** — see §Anisette & Apple ADI libraries. |
| GSA authentication | `icloud-auth` (SideStore/apple-private-apis) | Apple ID + password → GSA tokens. Handles 2FA challenge surfaces. |
| Apple Developer Services | `apple-dev-apis` (SideStore/apple-private-apis) | App ID create/list, device register, free-tier cert request, provisioning profile fetch. |
| Codesign helpers | `apple-codesign-wrapper` (SideStore/apple-private-apis) | Convenience wrappers used by SideStore stack; evaluate before adopting. |
| Device communication | `idevice` crate (jkcoxson) | Pure Rust, RSD / RemoteXPC support for iOS 17.4+ over USB **and** Wi-Fi. Used in production by StikDebug, CrossCode, Protokolle. Replaces ALL libimobiledevice CLI shell-outs in this app. |
| mDNS discovery | `mdns-sd` | Discovers `_remotepairing._tcp` / `_remoted._tcp` for Wi-Fi reachability. |

**Why not `isideload` (used by iLoader)?** It calls a *remote* anisette server. Note the nuance: a remote anisette server never sees the Apple ID password — credentials stay local in every option. We reject the remote path for the stronger privacy stance ("no third party in the loop"), no availability dependency, and to avoid the shared-server Apple-ID lock risk documented below. The cost we accept in exchange is local ADI library extraction (§Anisette & Apple ADI libraries).

**Why drop `ideviceinstaller`?** It is USB-only and the project's headline feature is Wi-Fi auto-refresh. iOS 17.4+ also requires RemoteXPC for most lockdown operations, and the system `ideviceinstaller` lags upstream. Using the `idevice` crate's native install API from Phase 3 onward is the only path that satisfies the brief's P0 Wi-Fi refresh requirement.

**Dependency declaration.** `apple-private-apis` is a workspace container of four member crates — it is *not* a publishable crate and has no releases on crates.io. We depend on the individual member crates by git rev, not on the workspace root:

```toml
omnisette              = { git = "https://github.com/SideStore/apple-private-apis", rev = "<pinned-sha>" }
icloud-auth            = { git = "https://github.com/SideStore/apple-private-apis", rev = "<pinned-sha>" }
apple-dev-apis         = { git = "https://github.com/SideStore/apple-private-apis", rev = "<pinned-sha>" }
apple-codesign-wrapper = { git = "https://github.com/SideStore/apple-private-apis", rev = "<pinned-sha>" }   # only if needed
```

**Staleness risk:** `apple-private-apis` master was last pushed 2024-11-14 and remains the de-facto reference. Treat as load-bearing-but-unmaintained — pin the git rev, vendor if necessary, plan a fork-and-patch budget. The `SigningProvider` trait is the insurance policy.

### Free Apple ID end-to-end credential flow

This is the sequence `signing/free_apple_id.rs` implements. Document it explicitly because no single crate orchestrates the whole flow.

0. **ADI provisioning (one-time, prerequisite)** — ensure `libstoreservicescore.so` + `libCoreADI.so` are present and a synthetic device is provisioned (see §Anisette & Apple ADI libraries). Fails with `AnisetteAdiUnavailable` if the setup step hasn't run.
1. **GSA login** — user enters Apple ID + password. `omnisette` produces anisette data (via the local ADI libs); `icloud-auth` performs GSA auth with anisette headers.
2. **2FA challenge (if required)** — flow surfaces `AppleAuth2FARequired`. Two completion paths:
   - Trusted-device code (default): user types 6-digit code from another Apple device.
   - SMS fallback: explicit user action ("send code via SMS") — uses Apple's secondary 2FA endpoint.
3. **Local key pair + CSR** — generate an RSA-2048 key pair on first use, store the private key in the keyring under `reside.signing.<account_hash>.key`. Build a CSR.
4. **Register cert** — submit CSR to Apple Developer Services via `apple-dev-apis`; receive a free-tier development certificate (~1 year validity). Persist cert metadata in `signing_profiles` and cert bytes in keyring.
5. **Register device** — register the target device's UDID. Counts toward the 10-device-per-7-days limit (see §Quotas).
6. **Resolve App ID** — look up an existing App ID for the desired bundle ID. If absent, create one. Counts toward 10-App-IDs-per-7-days. **Default policy: reuse aggressively** — refresh never creates a new App ID if a matching one exists.
7. **Fetch provisioning profile** — bind cert + App ID + device → 7-day profile. Persist in app data.
8. **Sign** — hand cert + key + profile + IPA to `apple-codesign`.
9. **After first install only** — surface "Open Settings → General → VPN & Device Management → Trust" UI on the host. Free-tier certs are untrusted by default until the user taps Trust on the device.

### Two expirations, not one

| Layer | Validity | Refresh action | UI signal |
|-------|----------|----------------|-----------|
| Provisioning profile | 7 days | Re-sign with existing cert + new profile (no Apple auth if anisette is fresh) | Expiration timeline per app |
| Development certificate | ~365 days | Full re-auth flow (likely 2FA), then re-issue cert, then re-sign | Banner: "Cert expires in N days — sign in to renew" |

`signing_profiles.cert_expires_at` tracks the cert. `installations.expiration_ts` tracks the profile.

### Apple Developer Services quotas

Tracked in SQLite (`apple_quota_events` table) per Apple ID, used to fail fast with actionable errors before calling Apple:

- **10 device registrations / rolling 7 days.**
- **10 App IDs / rolling 7 days.**
- **3 active sideloaded apps / device** at any time (enforced by Apple, surfaced as `BundleIdConflict`-style picker).

### IPA signing pipeline

`apple-codesign` signs `.app` bundles, not `.ipa` archives. The pipeline lives in `signing/ipa_pipeline.rs`:

1. Unzip `.ipa` to temp dir → locate `Payload/<Name>.app`. Preserve symlinks; do not normalize.
2. Parse `Info.plist`; capture original bundle ID, version, executable name.
3. **Bundle ID rewrite** (`signing/bundle_id.rs`) — prefix or replace per signing method; rewrite nested bundle IDs in `Info.plist` of embedded extensions/frameworks. Default: reuse existing bundle ID when possible to stay under App-ID weekly quota.
4. **Entitlements filter** (`signing/entitlements.rs`) — apply per-method allowlist (free Apple ID strips push, iCloud, IAP, Associated Domains, etc.; paid cert preserves).
5. Sign nested frameworks/dylibs/extensions bottom-up via `apple-codesign`.
6. Sign the top-level `.app`.
7. Repack as `.ipa` (zip, no compression on already-compressed assets; preserve Mach-O alignment; preserve symlinks).
8. Verify signature before handing to installer.

### Free Apple ID limits (surface in UI)

- 3 sideloaded apps per Apple ID per device.
- 10 device registrations per Apple ID per 7 days.
- 10 App IDs per Apple ID per 7 days.
- 7-day provisioning profile validity.
- ~1-year free-tier cert validity.

### Anisette & Apple ADI libraries

Generating valid anisette data locally is **not pure Rust**. `omnisette`'s local provider is an FFI shim over Apple's proprietary ADI libraries — `libstoreservicescore.so` and `libCoreADI.so` — which originate in the Apple Music Android APK. These libraries:

- **Cannot be redistributed** by an open-source project. ReSide must never bundle them. Instead, a first-run setup step downloads the Apple Music APK (user-initiated) and extracts only the two needed `.so` files into the app data dir. This is the same approach Dadoum/Sideloader uses.
- **Mean the build is not pure-Rust** — there is an FFI boundary (`libloading` or a thin `-sys` shim) and the libs are loaded at runtime, not link time.
- **Are version-fragile** — if Apple changes the ADI ABI in a future Apple Music release, extraction or calls can break. Pin a known-good APK version range; surface `AnisetteAdiUnavailable` / `AnisetteAdiIncompatible` cleanly.

**Account-lock hazard:** Apple is known to lock Apple IDs when anisette state is shared or inconsistent across many users. Because ReSide generates anisette *locally per machine* (not via a shared server), this risk is lower than the public-server path — but the synthetic device fingerprint must be **stable and unique per install** and must never be copied between machines. Document this in the UI ("don't clone your ReSide data dir to another computer").

Owned by `signing/adi.rs` (FFI + lib lifecycle) and `setup/adi_provision.rs` (APK download + extraction + version check).

### Anisette state persistence

`omnisette` produces anisette data tied to a synthetic device fingerprint. Losing it forces full re-auth (and a fresh 2FA challenge) on every signing run. Persistence rules:

- Store fingerprint + machine identifiers in the keyring under `reside.anisette.<account_hash>`.
- Refresh OTP-style anisette values on-demand per signing run; persist the long-lived fingerprint.
- Owned by `signing/free_apple_id.rs`. Not in SQLite — secrets only.

### Signing provider architecture

```
signing/mod.rs — SigningProvider trait (replaceable adapter)
├── free_apple_id.rs — omnisette + icloud-auth + apple-dev-apis (full credential flow above)
└── paid_cert.rs    — import p12 + .mobileprovision from disk
```

Both providers feed `signing/ipa_pipeline.rs`, which owns the unzip/patch/repack flow.

### Reference implementations (studied, not forked)

- **iLoader** (nab138/iloader) — Tauri + Rust sideloader, same stack. Study for: operation event pattern, secure storage fallback, iOS 17.4+ RPPairing branch.
- **Impactor** (claration/Impactor) — Full Rust sideloader (Iced UI). Study for GSA auth flow.
- **Dadoum Sideloader** (Dadoum/Sideloader) — D language, mature, all-in-one reference.
- **pymobiledevice3** (doronz88) — Python; study `tunneld` design for the RemoteXPC tunnel-keeper pattern (re-implemented in Rust here, not depended on).

---

## Workspace & Backend Structure

Cargo workspace so a future CLI binary (brief P2) can reuse all orchestration without duplicating it.

```
crates/
├── core/                          ← orchestration, signing, device, refresh
│   └── src/
│       ├── lib.rs
│       ├── error.rs               — AppError enum + taxonomy (see §Error Taxonomy)
│       ├── operation.rs           — event channel (UI layer adapts to Tauri events)
│       ├── secure_storage.rs      — keyring → filesystem fallback
│       ├── db.rs                  — SQLite setup + migration runner (sqlx::migrate!)
│       ├── ipa_store.rs           — content-addressed IPA store
│       ├── paths.rs               — XDG path resolver (see §Filesystem Layout)
│       ├── proc_lock.rs           — single-writer file lock between UI and agent
│       ├── setup/
│       │   ├── permissions.rs     — usbmuxd service, udev rules, group membership, Developer Mode, notification daemon
│       │   └── adi_provision.rs   — Apple Music APK download + extract libstoreservicescore.so/libCoreADI.so + version check
│       ├── device/
│       │   ├── mod.rs             — USB detect, pairing, iOS version + Developer Mode gates
│       │   └── pair_record.rs     — app-managed pair records (bypasses /var/lib/lockdown root requirement)
│       ├── transport/
│       │   ├── mod.rs             — Transport trait (v1: RemoteXpc; future: LegacyLockdown)
│       │   ├── remote_xpc.rs      — RSD + RemoteXPC strategy via idevice crate
│       │   ├── tunneld.rs         — long-running RSD tunnel manager (see §Tunnel Daemon)
│       │   └── mdns_discovery.rs  — _remotepairing._tcp / _remoted._tcp Wi-Fi discovery
│       ├── signing/
│       │   ├── mod.rs             — SigningProvider trait
│       │   ├── ipa_pipeline.rs    — unzip → patch → sign → repack
│       │   ├── bundle_id.rs       — bundle-ID rewriter
│       │   ├── entitlements.rs    — per-method allowlist filter
│       │   ├── free_apple_id.rs   — full credential flow (omnisette + icloud-auth + apple-dev-apis)
│       │   ├── adi.rs             — FFI shim over Apple ADI libs + synthetic device lifecycle
│       │   ├── paid_cert.rs
│       │   └── quota.rs           — Apple Developer Services quota tracking
│       ├── installer.rs           — IPA install via idevice crate (native, USB + Wi-Fi via tunnel)
│       └── refresh/
│           ├── mod.rs             — expiration tracking, refresh trigger
│           ├── scheduler.rs       — background job loop, profile vs cert refresh
│           └── agent.rs           — systemd user service + XDG autostart fallback
└── tauri-app/
    ├── src/                       — React + Vite + TS frontend (see §Frontend Structure)
    ├── src-tauri/
    │   ├── src/
    │   │   ├── lib.rs             — Tauri setup, invoke handlers (thin shims over core)
    │   │   └── redaction.rs       — Redactable impls for serialized payloads
    │   ├── tauri.conf.json        — capabilities + permissions (see §Phase 0a)
    │   └── capabilities/          — Tauri 2 capability files
    ├── package.json
    ├── vite.config.ts
    └── tsconfig.json
```

### Filesystem layout

All paths resolved via `core/src/paths.rs` using the `dirs` crate (no hardcoded `~` paths).

```
$XDG_DATA_HOME/reside/
├── data.db                          — SQLite (WAL mode)
├── ipas/<sha256>.ipa                — content-addressed IPA store
├── pair_records/<udid>.plist        — app-managed pair records
├── profiles/<profile_id>.mobileprovision
├── adi/                             — extracted libstoreservicescore.so + libCoreADI.so + provisioned device file
└── logs/                            — structured tracing logs (rotated, 30-day retention)
$XDG_CONFIG_HOME/reside/
└── config.toml                      — user-editable settings
$XDG_STATE_HOME/reside/
├── agent.pid                        — background agent PID + flock target
└── tunneld.sock                     — UDS for UI ↔ tunneld
$XDG_RUNTIME_DIR/reside/             — ephemeral tunnel state (if available)
~/.config/systemd/user/
├── reside-tunneld.service
└── reside-agent.{service,timer}
```

### SQLite schema

```sql
devices            (udid, name, ios_version, developer_mode_enabled, pairing_status, transport,
                    wifi_eligible, last_seen)
apps               (id, display_name, bundle_id, version, source_ipa_sha256, source_ipa_path, icon_path)
installations      (app_id, device_udid, signing_method, install_ts, expiration_ts,
                    cert_id, refresh_status, trust_prompt_shown)
signing_profiles   (id, signing_method, apple_id_hash, team_id, profile_metadata,
                    cert_expires_at, secret_ref)
apple_quota_events (apple_id_hash, event_type, ts)   -- event_type: device_registered | app_id_created
jobs               (id, installation_id, kind, next_run, last_run, retry_count, status)
activity_log       (ts, severity, operation, error_category, message)
```

Notes:
- `apps.source_ipa_path` points into the content-addressed IPA store. Required for background refresh — without the original IPA, re-signing is impossible.
- `apps.source_ipa_sha256` matches the store filename; enables dedup across installs.
- `devices.transport` is `remote_xpc` in v1 (column reserved for future `legacy_lockdown`).
- `devices.developer_mode_enabled` populated at pairing time; refresh fails fast with `iOSDeveloperModeOff` if false.
- `installations.cert_id` ties the install to the signing cert so cert-expiration sweeps can find affected installs.
- `installations.trust_prompt_shown` ensures the Trust-cert post-install UI fires only once per Apple ID per device.
- `jobs.kind` is `refresh_profile` or `refresh_cert` — they have different cost profiles (one needs Apple auth, one doesn't).
- `activity_log.error_category` keys into the §Error Taxonomy.
- Retention: delete IPA store files when no `installations` row references them; rotate `activity_log` at 30 days.

### Process coordination

UI and background agent are separate processes that share keyring, SQLite, and the active RSD tunnel. To avoid races:

- **Single-writer file lock** at `$XDG_STATE_HOME/reside/agent.pid` via `fs2::FileExt::try_lock_exclusive`. Whichever process is mutating state holds the lock.
- **SQLite in WAL mode** so reads from the other process never block.
- **Tunnel ownership** lives in the `reside-tunneld` service (always-on). UI and agent are tunnel *clients* over the UDS at `$XDG_STATE_HOME/reside/tunneld.sock`; only tunneld talks to the device.
- Agent yields to UI: when UI starts, it touches a sentinel; agent finishes its current job and sleeps until UI exits.

### Tunnel daemon (`transport/tunneld.rs`)

iOS 17.4+ developer services speak RemoteXPC over an IPv6 RSD tunnel — they cannot be reached over usbmux or a fresh TCP connection. Establishing the tunnel from cold takes seconds and can fail intermittently on Wi-Fi; doing it per-operation is fragile and rules out reliable 3am refresh.

ReSide runs a long-lived `reside-tunneld` systemd user service (mirroring the pymobiledevice3 `tunneld` design, re-implemented in Rust over the `idevice` crate). Responsibilities:

- Watch for devices via USB (RSD) and Wi-Fi (mDNS).
- Establish + maintain tunnels per device; reconnect on failure with exponential backoff.
- Expose a UDS at `$XDG_STATE_HOME/reside/tunneld.sock` returning live tunnel endpoints (host:port) to UI + agent clients.
- Bounded resource budget: < 50 MB RAM idle.

The refresh agent depends on this unit (`Requires=reside-tunneld.service` in the agent unit).

### Tauri command surface

```
run_setup_check        list_devices           pair_device
check_wifi_availability import_ipa            sign_ipa
install_ipa            list_installed_apps    schedule_refresh
run_refresh_now        get_activity_log       export_debug_bundle
submit_2fa_code        request_sms_2fa        get_tunnel_status
```

### Operation event protocol

Backend emits `operation_{id}` Tauri events with this payload:

```ts
{
  id: string;
  stage: "queued" | "preparing" | "authenticating" | "awaiting_2fa"
       | "signing" | "transferring" | "installing" | "verifying"
       | "trust_required" | "done" | "failed";
  progress: number;            // 0.0 — 1.0
  message?: string;            // human-readable, redacted
  error?: { category: string; remediation: string };
}
```

Frontend `OperationContext` subscribes by id; TanStack Query mutations resolve on `done`/`failed`. `awaiting_2fa` and `trust_required` are interactive stages: the UI shows a prompt and submits via `submit_2fa_code` or marks the operation user-acknowledged.

---

## Error Taxonomy

Every `AppError` variant maps to one category. Brief target: 95% of failures classified.

| Category | Remediation surfaced to user |
|----------|------------------------------|
| `UnsupportedIosVersion` | "ReSide requires iOS / iPadOS 17.4 or newer." |
| `iOSDeveloperModeOff` | "Enable Developer Mode: Settings → Privacy & Security → Developer Mode, then restart your device." |
| `iOSDeveloperCertUntrusted` | "On your iPhone: Settings → General → VPN & Device Management → tap your Apple ID → Trust." |
| `DeviceNotTrusted` | "Tap *Trust* on your iPhone and retry." |
| `DeviceLocked` | "Unlock your iPhone and retry." |
| `DeviceOffline` | "Connect via USB or check Wi-Fi." |
| `WifiUnreachable` | "Device not reachable on this network. Try USB." |
| `TunnelEstablishFailed` | "Could not establish a secure tunnel to the device. Restart `reside-tunneld` or reconnect via USB." |
| `UsbmuxdDown` | "Run: `sudo systemctl start usbmuxd`" |
| `PermissionsMissing` | "Add your user to the `plugdev` group / install udev rules." |
| `KeyringUnavailable` | "No system keyring detected. ReSide will use an encrypted filesystem fallback." |
| `AnisetteGenFailed` | "Local anisette generation failed — see logs." |
| `AnisetteAdiUnavailable` | "One-time setup needed: ReSide must download Apple's signing libraries. Start setup." |
| `AnisetteAdiIncompatible` | "Apple's signing libraries changed — update ReSide or re-run library setup." |
| `AppleAuthRateLimited` | "Apple is rate-limiting. Wait ~15 min." |
| `AppleAuth2FARequired` | Prompt for code (interactive). |
| `AppleAuthCredentialsInvalid` | "Wrong Apple ID or password." |
| `AppleAuthProtocolChanged` | "Apple changed their auth flow — app update needed." |
| `AppleDevCertGenFailed` | "Could not request a signing certificate from Apple — retry, or see logs." |
| `AppleDevDeviceRegLimitReached` | "You've registered 10 devices this week. Wait until the oldest registration ages out." |
| `AppleAppIdLimitReached` | "You've created 10 App IDs this week. Reuse an existing bundle ID, or wait." |
| `SigningCertExpired` | "Your signing certificate has expired — sign in again to renew." |
| `EntitlementsUnsupported` | "Some features may not work after signing." |
| `BundleIdConflict` | Offer reuse vs generate. |
| `InstallTransferFailed` | "Transfer to device failed — check USB cable or Wi-Fi." |
| `InstallVerifyFailed` | "Install completed but verification failed." |

---

## Frontend Structure

```
src/
├── App.tsx                    — root, keyboard shortcuts, modal orchestration
├── contexts/
│   ├── DeviceContext.tsx      — selected device state
│   ├── OperationContext.tsx   — operation event listener
│   ├── TunnelContext.tsx      — tunneld connection state
│   └── LogContext.tsx         — backend log stream
├── pages/
│   ├── SetupCheck.tsx
│   ├── Dashboard.tsx          — device status, expiration timeline, recent activity, tunnel indicator
│   ├── Sideload.tsx           — IPA import → sign → install wizard
│   └── Settings.tsx
└── components/
    ├── TwoFactorModal.tsx     — code entry + SMS fallback + "try again" / "stuck" link
    ├── TrustCertPrompt.tsx    — post-install instructions for free-tier first install
    ├── DeveloperModeGate.tsx  — blocks device flows until Developer Mode is on
    └── …                      — shadcn/ui-based shared components
```

### 2FA state machine

```
idle → awaiting_2fa
  ├─ code_submitted        → verifying → (success | invalid_code → awaiting_2fa)
  ├─ request_sms           → sms_pending → awaiting_2fa (with sms hint)
  └─ cancelled             → idle (operation fails AppleAuth2FARequired)
```

The modal must be dismissible. Cancellation leaves the operation in `failed` state with category `AppleAuth2FARequired` and a retry CTA on the dashboard.

**Key patterns:**
- TanStack Query for all device/signing/install async operations
- Zustand for wizard state, selected device, selected IPA
- Operation events from Rust backend drive install/refresh progress UI
- Sonner toasts for notifications

---

## What Sets This Apart from iLoader

| Feature | iLoader | This app |
|---------|---------|----------|
| Auto Wi-Fi refresh | ✗ | ✓ |
| Expiration tracking per (app × device) | ✗ | ✓ (SQLite) |
| Background agent | ✗ | ✓ (systemd user service + XDG autostart fallback) |
| Persistent RSD tunnel daemon | ✗ | ✓ (`reside-tunneld`) |
| Apple Developer quota tracking | ✗ | ✓ |
| Dashboard + activity log | ✗ | ✓ |
| Local-only anisette (no third-party server) | ✗ (remote server) | ✓ (omnisette + local ADI libs) |
| AUR packaging | ✗ | ✓ (post-v1.0) |

---

## Background Agent

- **Tunnel service:** `reside-tunneld.service` runs always; agent and UI are clients. Generated at first launch by `refresh/agent.rs` and installed at `~/.config/systemd/user/reside-tunneld.service`.
- **Refresh service:** `reside-agent.service` with a `reside-agent.timer` (6-hour OnCalendar). `Requires=reside-tunneld.service`, `After=reside-tunneld.service`.
- **Fallback for non-systemd hosts:** XDG autostart entries at `~/.config/autostart/reside-tunneld.desktop` and `~/.config/autostart/reside-agent.desktop`. Detect at runtime; UI surfaces which path is active.
- Jobs are idempotent and safe to retry. Failed refresh must not delete the currently installed app unless re-install succeeds.
- Agent yields to UI via the §Process coordination file lock.

---

## Permissions & Setup

`setup/permissions.rs` runs at first launch and on demand:

- `usbmuxd` installed + service active.
- udev rules present. Path varies by distro — check `/usr/lib/udev/rules.d/39-usbmuxd.rules` (Arch, Fedora) and `/lib/udev/rules.d/39-usbmuxd.rules` (some Debian/Ubuntu); accept either.
- Current user in `plugdev` (or `uucp` on Arch); offer copyable `usermod` command.
- Notification daemon present (libnotify-compatible — `org.freedesktop.Notifications` on D-Bus).
- Secret Service daemon reachable (gnome-keyring / KWallet) — otherwise use encrypted filesystem fallback, surface `KeyringUnavailable`.
- **Developer Mode** detected on first device connection via lockdown; if disabled, surface `iOSDeveloperModeOff` with on-device instructions and block install flows until re-detection succeeds.
- **Apple ADI libraries** present (`adi/libstoreservicescore.so` + `adi/libCoreADI.so`). If absent, offer the one-time `setup/adi_provision.rs` flow (download Apple Music APK → extract → provision synthetic device). Block free Apple ID signing until present; paid-cert signing is unaffected.
- `idevice` crate handles its own pair records inside `$XDG_DATA_HOME/reside/pair_records/`; we deliberately do *not* write to `/var/lib/lockdown/` so first-run never needs root.

AUR PKGBUILD `depends`: `usbmuxd libimobiledevice webkit2gtk-4.1 libnotify`. (Note: no `ideviceinstaller` — install is native via `idevice` crate.)

---

## Secrets & Redaction

- All secret material flows through `secure_storage.rs`. Never embedded in `activity_log` or operation events.
- `Redactable` trait in `core/src/error.rs`; all payloads serialized to UI or logs implement it. `Display` for sensitive types prints `<redacted>`. `thiserror` `#[from]` conversions must not capture secrets in their `Display` output — wrap upstream errors in redacted variants where needed.
- `export_debug_bundle` Tauri command zips: `activity_log` (last 30 days), structured tracing logs, device list (UDIDs hashed), `setup_check` output. Verifies no keyring values, anisette state, or Apple-ID strings appear in the zip before returning the path.

---

## Project Constraints

- **Solo developer** — use high-level crates over DIY; minimize surface area; resist refactor temptations between phases.
- **Open source from day one** — MIT license, GitHub Actions CI, AUR PKGBUILD stub in repo (published later).
- **Dependency license inventory** must be kept in `LICENSES.md`. Verified MIT-compatible at plan time: `apple-codesign` (MPL-2.0), `omnisette`/`icloud-auth`/`apple-dev-apis` (MPL-2.0), `idevice` (MIT), `keyring`, `sqlx`, `tauri` — all permissive.
- **Apple ADI libraries are NOT a dependency we ship.** `libstoreservicescore.so` / `libCoreADI.so` are Apple-proprietary, non-redistributable, and obtained per-machine at setup time from a user-downloaded Apple Music APK. The repo, releases, and CI never contain them. `LICENSES.md` must document this arrangement explicitly. As a result the free-signing path is not pure-Rust (runtime FFI via `libloading`).

---

## iOS Scope

v1 supports **iOS / iPadOS 17.4 or newer only.** Rationale: maintainer's test hardware (iPhone 17 Pro, iPad Pro M1) is firmly in the RemoteXPC era. Shipping a legacy lockdown transport we cannot verify end-to-end would risk silent regressions on the very devices we'd be claiming to support.

- README must lead with the version requirement.
- `setup/permissions.rs` and `device/mod.rs` reject devices reporting `< 17.4` with `UnsupportedIosVersion`.
- Developer Mode is required on iOS 16+. Since v1 targets 17.4+, Developer Mode is unconditional — `iOSDeveloperModeOff` blocks all install flows.
- `transport/mod.rs` is built as a trait from day one so a `LegacyLockdown` strategy can be contributed later without restructuring. PRs welcome but not maintainer-supported until someone with sub-17.4 hardware steps up.

---

## Local Dev Loop

The maintainer must be able to build, exercise, and validate the full pipeline on their own machine before any release tag is cut.

**Dev commands:**
- `pnpm install` (one-time, in `crates/tauri-app/`)
- `pnpm tauri dev` (from `crates/tauri-app/`) — Tauri 2 dev mode: Vite HMR for React + cargo watch for Rust. **Not** `cargo run -p tauri-app`.
- `cargo test -p reside-core` — headless pipeline tests (bundle-ID rewrite, entitlements filter, sign-and-verify against fixture IPA). No device needed.
- `cargo test -p reside-core -- --ignored` — device-required tests; gated so CI passes without hardware.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check` — CI parity locally.

**Time-warp env vars** (dev-only, behind `cfg(debug_assertions)`):
- `RESIDE_TEST_REFRESH_AT=<unix_ts>` — overrides scheduler's "now" so refresh fires immediately instead of waiting 6 days.
- `RESIDE_TEST_EXPIRES_IN=<seconds>` — overrides expiration stamp on next install so the next refresh cycle is observable in minutes.
- `RESIDE_TEST_CERT_EXPIRES_IN=<seconds>` — same idea for cert-expiration sweep.

**`docs/MANUAL_TEST.md`** lives in repo and is the maintainer's pre-release checklist. Required passing runs on **both** the iPhone 17 Pro and iPad Pro (M1) before any GitHub release tag:

1. Fresh install → setup check → pair via USB → device appears.
2. Developer Mode disabled → ReSide surfaces `iOSDeveloperModeOff` with correct instructions. Enable → restart device → reconnect → continue.
3. Paid-cert signing path (no Apple auth) → install fixture IPA → app launches on device.
4. First-run ADI setup → download Apple Music APK → extract `.so` libs → provision synthetic device → anisette generates successfully.
5. Free Apple ID signing path → 2FA flow (trusted-device code) → install → Trust-cert prompt shown → user trusts → app launches.
6. Free Apple ID signing path → 2FA via SMS fallback → install → app launches.
7. `reside-tunneld` running → kill it → confirm UI shows tunnel-down → systemd restarts it → state recovers.
8. Wi-Fi reachability check passes for both devices after USB pairing (mDNS discovery succeeds).
9. Trigger refresh via `RESIDE_TEST_EXPIRES_IN=120` → confirm re-sign + reinstall over Wi-Fi → desktop notification fires; no 2FA needed.
10. Trigger cert refresh via `RESIDE_TEST_CERT_EXPIRES_IN=120` → confirm full re-auth path fires.
11. Kill Wi-Fi mid-refresh → confirm retry/backoff → prior install still present.
12. `export_debug_bundle` → grep output for Apple ID / unhashed UDIDs / anisette tokens (must be empty).

CI cannot run any of this. Only the maintainer can sign off on a release.

---

## Phased Implementation

Each phase ends with a **Definition of Done** checklist. Do not advance until every item passes — this is the AI executor's stop-gate.

### Phase 0a — Repo skeleton (one afternoon)
1. `cargo create-tauri-app` → restructure into workspace (`crates/core`, `crates/tauri-app`).
2. Frontend (`crates/tauri-app/`): Vite, shadcn/ui, Tailwind CSS, Zustand, TanStack Query.
3. Tauri 2 capabilities: write `crates/tauri-app/src-tauri/capabilities/default.json` granting only the permissions this app actually needs (FS read for IPA picker, shell exec for `systemctl --user`, notifications, dialog, OS-info, network). Do not enable `allowlist:**`.
4. Repo: LICENSE (MIT), `LICENSES.md`, README with iOS 17.4+ requirement, `.github/workflows/ci.yml` (build + clippy + fmt + Arch and Fedora matrix using `webkit2gtk-4.1`), `AUR/PKGBUILD` stub (not yet published), `tests/fixtures/README.md`, `docs/MANUAL_TEST.md` (initially empty checklist, populated as features land).

**DoD:** `pnpm tauri dev` launches a blank window; `cargo build --workspace` clean; `cargo clippy -D warnings` clean; CI green on both Arch and Fedora.

### Phase 0b — Foundational primitives
1. Rust: module stubs across the tree, `AppError` enum populated with all §Error Taxonomy variants.
2. `core/src/operation.rs` event channel + Tauri adapter.
3. `core/src/paths.rs` XDG resolver covering all paths in §Filesystem Layout.
4. SQLite migrations via `sqlx::migrate!` for all tables in §SQLite schema.
5. `secure_storage.rs` with keyring → encrypted filesystem fallback.
6. `proc_lock.rs` single-writer file lock + integration test (two processes, one wins).
7. `ipa_store.rs` skeleton (content-addressed store).
8. `Redactable` trait + smoke test that no `Debug`/`Display` of secrets leaks the value.

**DoD:** Migration runs cleanly on fresh DB; keyring round-trip works under gnome-keyring and falls back gracefully when unavailable; `proc_lock` test passes; redaction smoke test passes.

### Phase 1 — Device Detection *(testable without Apple auth)*
- `transport/remote_xpc.rs` + `transport/tunneld.rs`: RSD tunnel establishment via `idevice` crate. Run tunneld inline (not yet as a service).
- `transport/mdns_discovery.rs`: Wi-Fi discovery of `_remotepairing._tcp`.
- `device/mod.rs` + `device/pair_record.rs`: USB detect, pairing flow, Developer Mode + iOS version gates.
- `setup/permissions.rs`: usbmuxd, udev, group, notification daemon, keyring checks with copyable fix commands.
- Tauri commands: `list_devices`, `pair_device`, `check_wifi_availability`, `run_setup_check`, `get_tunnel_status`.
- Frontend: setup checklist + device list + connection status + tunnel indicator.

**DoD:** iPhone 17 Pro + iPad Pro paired over USB → both appear with iOS version + Developer Mode state; Wi-Fi reachability succeeds for both after pairing; simulated `< 17.4` device rejected with the right error; setup check correctly diagnoses a stopped `usbmuxd` and missing group on a fresh Arch VM.

### Phase 2 — IPA Import + Signing
- `ipa_pipeline.rs`: unzip, metadata extract (`Info.plist` via `plist` crate), `bundle_id.rs`, `entitlements.rs`, sign, repack, verify.
- `paid_cert.rs` first (no Apple auth required, no ADI libs needed, easier to test).
- `setup/adi_provision.rs` + `signing/adi.rs`: Apple Music APK download, `.so` extraction, version check, synthetic-device provisioning, FFI lifecycle.
- `free_apple_id.rs` implementing the full §Free Apple ID end-to-end credential flow (steps 0–8).
- `signing/quota.rs` writing `apple_quota_events` on every device-register / app-id-create call; reading to fail fast.
- Anisette persistence in keyring.
- Frontend: IPA import UI, signing method picker, free-tier limits banner, 2FA modal (`TwoFactorModal.tsx`).

**DoD:** Fixture IPA signs end-to-end with paid cert path; ADI provisioning extracts both `.so` files from a real Apple Music APK and generates anisette; free Apple ID path completes through 2FA (trusted device + SMS fallback both verified manually); quota table records events; quota-exceeded path surfaces `AppleDevDeviceRegLimitReached` without calling Apple.

### Phase 3 — Install + Inventory
- `installer.rs`: native IPA install via `idevice` crate (no `ideviceinstaller` shell-out). Same code path serves USB and Wi-Fi.
- Write `apps` + `installations` rows; store IPA via `ipa_store.rs`.
- `TrustCertPrompt.tsx` shown on first free-tier install per (Apple ID, device).
- Frontend: progress UI (operation events), dashboard with expiration timeline.

**DoD:** Sign + install fixture IPA over USB → app appears on device → `installations` row + source IPA in store; first free-tier install surfaces Trust prompt and app launches after user trusts; install over Wi-Fi (with `reside-tunneld` running inline) succeeds.

### Phase 4 — Background Refresh
- Promote tunneld from inline to dedicated `reside-tunneld.service`. Generate unit file via `refresh/agent.rs`.
- `refresh/scheduler.rs`: profile-refresh path (no Apple auth) and cert-refresh path (full re-auth). Distinct `jobs.kind`. Idempotent + retry with backoff.
- `refresh/agent.rs`: `reside-agent.service` + `.timer`, with XDG autostart fallback.
- Desktop notifications via Tauri plugin.
- Process coordination: agent respects UI's file lock; UI surfaces "agent paused while UI is open" if relevant.
- Frontend: expiration timeline (profile + cert), refresh status, agent install/uninstall controls.

**DoD:** Manual test items 6–10 all pass on both devices; profile-refresh runs end-to-end without 2FA; cert-refresh fires when `RESIDE_TEST_CERT_EXPIRES_IN` short-circuits; killing Wi-Fi mid-refresh leaves prior install intact and triggers backoff.

### Phase 5 — Polish + GitHub Release (v1.0)
- Activity log UI with category filtering.
- `export_debug_bundle` command + UI; redaction verification at export time.
- README polish; populate `docs/MANUAL_TEST.md` with all 11 checks from §Local Dev Loop.
- GitHub Actions release workflow: tagged builds produce signed `.tar.gz` (and `.deb` if cheap) attached to the release.
- **Release gate:** maintainer runs the full `docs/MANUAL_TEST.md` checklist on both iPhone 17 Pro and iPad Pro (M1). No tag without a passing run recorded in the PR description.
- v1.0 ships as a GitHub Release only. AUR + Flatpak deferred.

**DoD:** Release workflow produces a downloadable artifact that installs and runs on a fresh Arch VM with only the documented `depends` installed.

### Phase 6 — AUR + broader packaging (post-v1.0)
- Triggered by dogfooding feedback and early-adopter reports from the GitHub Release.
- Finalize `AUR/PKGBUILD` against the v1.x tag; publish to AUR.
- AppImage / Flatpak evaluation. Flatpak sandboxing constraints are no longer a blocker (native `idevice` was adopted from Phase 3), but Wayland portal access for the file picker and D-Bus access to notifications must be configured in the Flatpak manifest.

---

## Testing Strategy

- `tests/fixtures/`:
  - **`HelloReSide.ipa`** — a known-good fixture IPA built in CI from an MIT-licensed minimal SwiftUI app in `tests/fixtures/HelloReSide-src/` (so the binary is reproducible and redistributable). If cross-building from Linux proves impractical, vendor a pre-built copy under the project's own MIT license with provenance noted in `tests/fixtures/README.md`.
  - Captured pairing record (UDID redacted).
  - Recorded `idevice` API responses for installer-layer tests.
- Pipeline tests (`ipa_pipeline`, `bundle_id`, `entitlements`) run in CI without a device.
- Device-required tests gated by `#[ignore]`; manual test matrix lives in `docs/MANUAL_TEST.md` and covers both iPhone 17 Pro + iPad Pro (M1).
- Mock `SigningProvider` and mock `Transport` for end-to-end orchestration tests (no Apple auth, no device in CI).
- Distro matrix in CI: Arch (primary) + Fedora (catches packaging assumptions early). Both must install `webkit2gtk-4.1` and `libnotify`.

---

## Key Dependencies (`Cargo.toml`)

```toml
tauri = { version = "2", features = [...] }
idevice = "<pin to latest at scaffold time>"

apple-codesign = "0.29"   # latest as of plan (2024-11); check for newer before pinning

omnisette       = { git = "https://github.com/SideStore/apple-private-apis", rev = "<pin>" }
icloud-auth     = { git = "https://github.com/SideStore/apple-private-apis", rev = "<pin>" }
apple-dev-apis  = { git = "https://github.com/SideStore/apple-private-apis", rev = "<pin>" }

sqlx     = { version = "0.8", features = ["sqlite", "runtime-tokio", "migrate"] }
keyring  = "3"
mdns-sd  = "0.11"
libloading = "0.8"             # runtime FFI load of Apple ADI .so libs (omnisette local provider)
zip      = "2"                 # for IPA unpack/repack
plist    = "1"
fs2      = "0.4"               # process file lock
dirs     = "5"                 # XDG path resolution
tokio    = { version = "1", features = ["full"] }
serde    = { version = "1", features = ["derive"] }
thiserror = "2"
tracing   = "0.1"
tracing-subscriber = "0.3"
```

At scaffold time, the executor must:
1. Verify `idevice` latest version on crates.io and pin.
2. Verify `apple-codesign` latest and pin.
3. Find a known-good commit on `apple-private-apis` master (HEAD as of scaffold day is fine) and pin all three member crates to the same SHA.
4. Run `cargo build` clean before continuing.

---

## Verification Checkpoints

- `cargo build` passes cleanly across the workspace with no warnings.
- **Permissions check:** `run_setup_check` correctly detects missing `usbmuxd`, missing group membership, missing notification daemon, and missing keyring on a fresh Arch VM.
- **Device detection:** iPhone 17 Pro + iPad Pro (M1) → `list_devices` returns UDID + name + iOS version + Developer Mode state + RemoteXPC transport for both.
- **Version gate:** simulated `< 17.4` device → `UnsupportedIosVersion`.
- **Developer Mode gate:** device with Developer Mode disabled → `iOSDeveloperModeOff` with correct on-device instructions.
- **Tunnel daemon:** `reside-tunneld` establishes tunnels for USB + Wi-Fi devices, survives device unplug/replug, restarts cleanly via systemd.
- **IPA pipeline (no device):** test fixture IPA → bundle-ID rewrite + entitlements strip + sign with known p12 + verify — runs in CI.
- **Quota fail-fast:** simulated 10th-device-this-week registration is rejected locally with `AppleDevDeviceRegLimitReached` without calling Apple.
- **End-to-end USB install:** sign + install over USB → app appears on device → `installations` row + source IPA in store.
- **End-to-end Wi-Fi install:** same, but via Wi-Fi transport through `reside-tunneld`.
- **Trust cert prompt:** first free-tier install per (Apple ID, device) shows Trust prompt; second install does not.
- **Profile-refresh:** background agent re-signs before profile expiration → desktop notification received; no 2FA required; failed-refresh leaves prior install intact.
- **Cert-refresh:** simulated cert expiration triggers full re-auth path with 2FA modal.
- **Process coordination:** UI launch while agent is mid-refresh → agent yields cleanly; both processes see consistent SQLite state.
- **Debug bundle:** `export_debug_bundle` output grep'd for Apple ID strings, UDIDs (unhashed), anisette tokens — must be empty.

---

## Known Foot-Guns for AI Executors

Read this section before changing anything not explicitly described above.

- **Do not auto-bump deps.** Version pins are deliberate. If a dep refuses to build, surface the error; do not silently upgrade.
- **Do not replace pinned git revs.** `apple-private-apis` member crates are pinned to a specific SHA. If you must change it, document why and update all three pins together to the same SHA.
- **Do not depend on `apple-private-apis` directly.** It is a workspace, not a crate. Depend on `omnisette` / `icloud-auth` / `apple-dev-apis` individually.
- **Do not reintroduce `ideviceinstaller`.** It is USB-only and conflicts with the Wi-Fi refresh requirement. Native install via the `idevice` crate is the chosen path from Phase 3 onward.
- **Do not write to `/var/lib/lockdown/`.** The app manages its own pair records in `$XDG_DATA_HOME/reside/pair_records/` to avoid requiring root on first run.
- **Do not rewrite the `SigningProvider` or `Transport` traits without permission.** Both are the project's insurance policy against upstream churn — they are designed to be replaceable.
- **Do not add features outside the current phase.** Each phase has a Definition of Done. Stop when it passes; do not opportunistically scaffold the next phase.
- **Do not add `eprintln!`/`println!` to backend code.** All diagnostics go through `tracing` so `export_debug_bundle` can capture and redact them.
- **Do not store secrets in SQLite.** `signing_profiles.secret_ref` is a *reference* to a keyring entry, not the secret itself. Apple ID strings are hashed before persistence.
- **Never bundle or commit Apple's ADI libraries.** `libstoreservicescore.so` / `libCoreADI.so` are Apple-proprietary and must not be redistributed. They are downloaded + extracted on the user's machine at setup time only. Do not add them to the repo, the release artifact, or CI fixtures.
- **The free-signing path is not pure Rust.** It loads Apple's ADI libs over FFI at runtime. Do not "simplify" by ripping out the FFI/`libloading` boundary — there is no pure-Rust anisette implementation.
- **Do not copy the anisette/ADI device file between machines.** The synthetic device fingerprint must be unique per install or Apple may lock the account.
- **Do not skip Tauri 2 capabilities config.** Default-deny capabilities will block half the app at runtime. Explicit capability files in `crates/tauri-app/src-tauri/capabilities/` are mandatory.
- **Do not collapse the two refresh kinds.** Profile refresh (7-day) and cert refresh (~1-year) have very different cost profiles. Keep `jobs.kind` distinct.
- **Treat Apple Developer Services as a quota-limited external API.** Always check `apple_quota_events` *before* calling, and always log a quota event *after* a successful call.
- **`apple-private-apis` is load-bearing-but-unmaintained.** If a member crate breaks against current Apple endpoints, surface clearly. Do not silently rewrite Apple auth in-app — that's a scope explosion. Open an issue and pause.
