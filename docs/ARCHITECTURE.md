# ReSide Architecture

The durable, current source of truth for how ReSide works. If anything here
disagrees with `plan.md` or `Product-Brief.md`, **this file wins** — those are
older and partly describe an abandoned approach (see [Live vs parked](#live-vs-parked-code)).

## What ReSide is

A Linux-first desktop app (Tauri 2 + React/TS front end, Rust `reside-core`
back end) that signs, sideloads, and **auto-refreshes** iOS apps on the user's
own iPhone/iPad. It is the **automation, Wi-Fi, and reliability layer over a
proven signer** — it does *not* reimplement Apple's signing stack.

## The pivot (read this first)

ReSide originally tried to reimplement Apple's developer-services / signing
protocol natively in Rust. That path was **abandoned**. ReSide now **drives a
forked [Dadoum Sideloader](https://github.com/Dadoum/Sideloader) CLI** (GPL-3.0,
written in D) as a child process, and adds the three things that fork lacks and
that motivated the whole project:

1. **Auto-refresh** — free Apple-ID signatures expire after 7 days; ReSide
   re-signs/reinstalls before they lapse, unattended.
2. **Wi-Fi** — sign/install over the network after the first USB pairing.
3. **Reliable signing** — one validated signer, one-click recovery.

## Live vs parked code

This is the highest-value thing to understand before editing `reside-core`.
The live signing flow is **`signer.rs`** (drives the fork). The `signing/`
module and `setup/adi_provision.rs` are the **superseded native attempt** —
still compiled, but **not wired into the live app** (the Tauri backend imports
`reside_core::signer`, never `reside_core::signing`). Their doc comments still
say "Phase 2"/"future"/"insurance policy"; treat that as historical, not a roadmap.

| Module | Status | Role |
|--------|--------|------|
| `signer.rs` | **LIVE** | Drives the forked Sideloader CLI: credential storage, spawn-with-stdin, 2FA classification, USB/Wi-Fi install. The real signing path. |
| `transport/` (`muxer`, `mdns_discovery`, `remote_xpc`, `tunneld`) | **LIVE** | Device transport + Wi-Fi bridge (netmuxd) wiring. |
| `refresh/` (`scheduler`, `agent`, `mod`) | **LIVE** | Trigger-agnostic refresh engine + unattended systemd agent. |
| `installs.rs`, `installer.rs`, `ipa_store.rs`, `ipa_meta.rs` | **LIVE** | Install inventory + content-addressed IPA store. |
| `secure_storage.rs`, `locate.rs`, `operation.rs`, `paths.rs`, `db.rs`, `proc_lock.rs` | **LIVE** | Credential keyring, helper-binary resolution, op-event protocol, paths, SQLite, single-instance lock. |
| `device/`, `setup/permissions.rs`, `setup/mod.rs` | **LIVE** | Device model + first-run permission checks. |
| **`signing/`** (`free_apple_id`, `adi`, `ipa_pipeline`, `paid_cert`, `quota`, `entitlements`, `bundle_id`) | **PARKED** | The abandoned native signing/Apple-auth pipeline. Replaced by the fork. Kept for reference / the `native-signing-path` branch. |
| **`setup/adi_provision.rs`** | **PARKED** | ReSide's own ADI provisioning — unused; the fork downloads the Apple component itself. |

## Runtime shape: ReSide + two spawned helpers

ReSide ships and spawns two external binaries (kept **beside** the app
executable; `locate.rs` resolves "next to current_exe" → absolute path, which is
also what lets the systemd agent self-configure):

- **`sideloader`** (GPL-3.0) — the forked Dadoum signer. Build is fiddly: pinned
  **LDC 1.34** + a `libxml2.so.2` symlink (system ldc 1.42 fails on botan).
- **`netmuxd`** (LGPL-2.1) — on-demand Wi-Fi mux bridge. Plain `cargo build
  --release`; shipped **unmodified** upstream (`jkcoxson/netmuxd` @ `1c7dfd1`).

### The fork's two ReSide patches (branch `reside-automation`)

1. **Non-interactive login** (`c9b65db`) — with `RESIDE_NONINTERACTIVE=1` the CLI
   reads Apple ID + password from **stdin** (never argv/env → no `/proc` leak).
   If Apple demands 2FA it prints `RESIDE_2FA_REQUIRED` and exits `2`, so ReSide
   can prompt the user and re-invoke. A trusted device skips 2FA — this is what
   makes unattended refresh possible. This patch is structural; without it there
   is no automation layer.
2. **TLS-verify-on** (`673db69`) — upstream disabled cert verification on the
   ~150 MB Apple-component download (native code that then gets loaded/run → an
   injection vector). ReSide turns it back on. Candidate to upstream to Dadoum.

## First-run reality (every new user)

1. **~150 MB Apple component download.** To sign with a free Apple ID, Apple's
   proprietary ADI libraries (`libstoreservicescore.so`, `libCoreADI.so`) are
   required. The fork downloads the Apple Music APK from **Apple's own CDN**
   (TLS-verified) and extracts them on the user's machine. **Never bundled,
   committed, or shipped** by ReSide — see `LICENSES.md`.
2. **2FA on first sign-in.** One-time Apple device-trust, not per-app.
   Grandfathered/trusted devices skip it.
3. **Keyring optional.** 3-tier creds: "on this device" (keyring-persisted,
   enables the agent) · "just this session" (in-memory) · "don't remember" (ask
   each time). No plaintext fallback (the `File` backend is `cfg(test)`-only).

## Repos & branches

- **ReSide** (this repo). The live line is **`main`** (the GitHub default that
  `origin/HEAD` tracks; the public v0.4.x releases ship from it). Work on
  short-lived feature branches and PR into `main` (see PRs #4–#11). The abandoned
  native-signing work is no longer in the active history — `signer.rs` is the
  live path (see "Live vs parked code" above).
- **Fork** at `../sideloader-fork`, branch `reside-automation`. Its `origin` is
  upstream `Dadoum/Sideloader` — **never push the fork there**; push to the
  user's own fork repo.

## Build / run / gates

**Prerequisites:** Rust via rustup (the `rust-toolchain.toml` pin — 1.95.0 +
clippy/rustfmt — installs on first `cargo` run, so all environments match);
Node 18+ and pnpm 10+; and Tauri's Linux system libs (WebKitGTK 4.1, GTK 3,
libsoup 3, a C toolchain, `pkg-config`). No `-dev` split on Arch — the runtime
packages carry the headers.

**The esbuild / pnpm gotcha (don't relearn this):** recent pnpm (11.x) won't run
esbuild's postinstall build script and then *aborts* `pnpm build` / `pnpm
tauri:dev` on its pre-run dependency check — `ERR_PNPM_IGNORED_BUILDS`. esbuild
ships its native binary (`@esbuild/<platform>`) prebuilt, so that script is
unnecessary and the abort is a false positive. A committed `verifyDepsBeforeRun:
false` in `crates/tauri-app/pnpm-workspace.yaml` disables the check so the
commands below work unmodified; the "ignored build scripts: esbuild" line at
`pnpm install` time is cosmetic. (Equivalent one-off: pass
`--config.verifyDepsBeforeRun=false`. Note: `onlyBuiltDependencies` in that same
file is *not* honored by this pnpm version, and the setting is read from
`pnpm-workspace.yaml`, not `.npmrc`.)

From the repo root, keep all four green:

```sh
cargo test -p reside-core
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
( cd crates/tauri-app && pnpm build )      # the TS gate
```

GUI: from `crates/tauri-app/` run **`pnpm tauri:dev`** (the `:dev` script sets
the Wayland flags `WEBKIT_DISABLE_COMPOSITING_MODE` + `WEBKIT_DISABLE_DMABUF_RENDERER`
this machine needs — plain `pnpm tauri dev` will misrender). **Rust changes need
a `tauri:dev` restart.** Helpers auto-found in `target/debug/`, else set
`RESIDE_SIDELOADER_BIN` / `RESIDE_NETMUXD_BIN`.

Release packaging: `packaging/` (`build-tarball.sh`, `install.sh`, `RELEASING.md`).
Clean-room test helper: `scripts/reside-reset-newuser.sh` (resets to new-user state;
backs up first; does not touch `/var/lib/lockdown` pairing).

## Gotchas (hard-won; do not relearn the hard way)

- **Device pairing uses the ONE usbmuxd trust store** (fixed `077e230`). Do not
  reintroduce a private pair store — it clobbers the signer's pairing.
- **Wi-Fi install** = on-demand netmuxd sidecar via `USBMUXD_SOCKET_ADDRESS`;
  set for Wi-Fi, cleared for USB. The fork + refresh engine are transport-agnostic.
- **Refresh agent**: resolved helper paths are baked into the systemd unit only
  when **absolute** (a unit starts with an empty PATH). Watch the **stale-unit**
  trap — regenerate the unit after changing helper locations.
- **Frontend theming**: never put `data-theme` and a `dark:` Tailwind utility on
  the same node. The dark-mode bug was a `GnomeWindow` self-selector issue.
- **Diagnostics via `tracing`, never `println!`/`eprintln!`** (so debug-bundle
  export can capture + redact).
- **Never bundle/commit Apple's ADI libs.** **Never copy the anisette/ADI device
  file between machines** (the synthetic fingerprint must be unique or Apple may
  lock the account).
- **Don't auto-bump pinned deps** (Rust crates, the ldc 1.34 fork toolchain).
- Apple Developer Services is quota-limited; transient `-22406` → recover via
  "Refresh now". Free Apple IDs cap at ~2 active dev certs (the cert-management
  UI gap — see `docs/ROADMAP.md`).

## iOS scope

Requires iOS/iPadOS using the modern RemoteXPC / RSD transport. The README
states a 17.4 minimum (the transport threshold); validated on the user's iPhone
(iOS 26.5) and iPad Pro M1. The exact floor is unverified — confirm before
making hard claims to users.
