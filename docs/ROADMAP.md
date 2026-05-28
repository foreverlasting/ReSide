# ReSide Roadmap

Prioritized work for the next agent. Read `docs/ARCHITECTURE.md` first. Each item
says **why**, **scope**, **where**, and **done-when**. Do them top-down; don't
opportunistically scaffold lower items.

## Current state (2026-05-28)

Functionally complete and hardware-validated: sign + install over USB **and**
Wi-Fi, auto-refresh engine + unattended systemd agent, 3-tier credentials, UX
redesign. Plus a system-tray icon (left-click toggles window, right-click =
Show / Quit).

**Public as of 2026-05-28.** The three go-live steps are done:

1. `foreverlasting/ReSide` is public.
2. `foreverlasting/Sideloader` (the patched signer fork — branch
   `reside-automation`, two patches: non-interactive login + TLS-verify) is
   public, satisfying GPL-3.0 source obligation for the signer binary the
   tarball ships.
3. **v0.4.1 release is published** at
   <https://github.com/foreverlasting/ReSide/releases/tag/v0.4.1> with
   `ReSide-0.4.1-linux-x86_64.tar.gz` (sha256
   `6312a2dfa81029b0f220235f7f984efc798e58e2bf54a6231875e1934f70bf57`).

v0.4.1 supersedes a never-published v0.4.0 draft. Hardware testing on KDE
Plasma 6 / CachyOS / Wayland exposed four bugs in v0.4.0 (Wayland launch
silently failed, KDE menu icon missed, tray init panicked without
libappindicator, mock-gallery screens read as half-built); all fixed in
PRs #5–#8, version bump in #9.

## 1. Certificate management + credential settings UI  ← start here

**Why:** the biggest new-user cliff. Free Apple IDs cap at ~2 active dev certs;
when a user hits it, signing fails and the app offers **no way out**. There's
also no in-app way to change or forget stored credentials.

**Scope:** a Settings screen that (a) lists current certs and revokes one, and
(b) changes/forgets the saved Apple ID. The CLI already does the cert half
(`sideloader cert list|revoke`, used live), so this is "surface existing
capability," not new signing logic. Credential half uses
`reside_core::signer::{store,clear}_credentials`.

**Where:** new Tauri commands wrapping the fork's cert subcommands in
`crates/core/src/signer.rs` + `crates/tauri-app/src-tauri/src/lib.rs`; a Settings
view in the React front end. Mind the theming gotcha (no `data-theme` + `dark:`
on one node).

**Done when:** a user at the 2-cert cap can revoke from the UI and immediately
sign again; can switch Apple IDs without editing files. Validate on hardware.

**Status (2026-05-26): built, four gates green, NOT yet hardware-validated.**
- Core: `signer::{list_certs, revoke_cert}` drive the fork's `cert list|revoke`
  (parsing its human output — no new fork patch); `parse_cert_list` is unit-tested.
- New error `AppleCertLimitReached` (category + remediation), classified off Apple's
  "already have a current … certificate" text (portal code 7460).
- Tauri commands `list_certificates` / `revoke_certificate`; IPC `CertInfo`.
- UI: `screens/Settings.tsx` (a new "settings" overlay; the sidebar/dashboard
  "settings" nav now opens it, not Setup) with a Certificates list+revoke and an
  Apple ID change/forget form. Plus the chosen **auto-prompt at the cap**: when an
  install/refresh fails with `AppleCertLimitReached`, the modal shows a "Manage
  certificates" button that jumps to Settings.
- **Known gap:** if `cert list`/`revoke` triggers a fresh 2FA challenge, Settings
  only shows the remediation text — there's no inline code entry (a trusted device
  skips 2FA, so this is rare). Wire a 2FA prompt here if hardware shows it's needed.
- **Validate on hardware:** with ≥1 cert on the account, open Settings → see the
  list; revoke one → it disappears and a subsequent sign works; force the cap to
  confirm the install-modal auto-prompt appears and lands on Settings.

## 2. Pre-public polish — **DONE 2026-05-28**

All five bullets shipped (absorbed into the v0.4.1 hardening PRs #5–#8 and
follow-ups). Verified against current code 2026-05-28:

- **`mdns_sd` ERROR log noise** — no `tracing::error!` left in
  `transport/mdns_discovery.rs`; only the legitimate `warn!` on a failed browse.
- **Mock gallery screens** — `App.tsx` gates `<Gallery />` to non-Tauri runs
  (`pnpm dev` in a plain browser); live users only ever see `<ReSideApp />`.
  Gallery pages carry an on-screen "Design preview · mock data" label.
- **Code-doc pass for live-vs-parked** — every file under `signing/` plus
  `setup/adi_provision.rs` opens with a `⚠️ **PARKED**` header pointing the
  reader at `signer.rs` as the live path.
- **iOS minimum claim** — README softened to "Recommended: iOS / iPadOS 17.4
  or newer" with an explicit "iOS 17.0–17.3 may work but is…" hedge.
- **Stale `icons/icon.ico`** — file removed; `icons/` is png + svg only.

## 3. Wi-Fi devices in the Devices rail — **DONE 2026-05-28**

**Why:** Wi-Fi is the headline feature, but the Devices rail listed USB only —
the thing you're proudest of was invisible.

**Approach:** C (hybrid). With the cable out, a soft "an iPhone is reachable
over Wi-Fi" banner appears in the rail's empty state (the existing 3-second
mDNS scan in `transport/mdns_discovery.rs`, now polled passively); a "Connect
over Wi-Fi" button spins netmuxd up on demand via the new
`transport::muxer::resolve_wifi_devices`, reads the device's name + iOS
version through it, caches the resolved `DeviceInfo` in a session-only
`transport::wifi_cache`, then tears netmuxd down. `device::list_devices`
merges USB ∪ Wi-Fi cache (USB wins on UDID conflicts). Hardware-validated;
the **on-demand teardown** the Wi-Fi-install slice depends on is preserved.

**Gotcha worth re-reading:** Linux `usbmuxd` is udev-activated and *exits*
when no cable is attached; `device::list_devices` therefore treats a
connection failure as "no USB devices" rather than fatal, so the Wi-Fi
cache can still surface a resolved card. Don't regress that.

## 4. AUR packaging

**Why:** the planned real distribution channel for Arch/CachyOS users; sources
from the GitHub Release. **Scope:** a `PKGBUILD` that pulls the release tarball
(or builds from source, incl. the fork's pinned-ldc build). **Done when:**
`makepkg -si` installs a working ReSide on a clean Arch box. Sequence after the
release is public.

## 5. Release automation (CI builds the tarball on tag)

**Why:** releases are hand-built today; automation makes them reproducible.
**Scope:** a GitHub Action that runs the gates and `packaging/build-tarball.sh`
on a `v*` tag, attaching the artifact. **Caveat:** the D signer's pinned-toolchain
build is the genuinely hard part to reproduce in CI — may need a prebuilt-helper
cache or a container image. Don't block the first manual release on this.

## 6. Upstream the TLS-verify fix to Dadoum

**Why:** it's a real security fix; acceptance shrinks the fork to one patch
(the non-interactive login) and is good open-source citizenship. **Scope:** open
a PR to `Dadoum/Sideloader` from `673db69`.

## Standing constraints

User is **not a developer** — explain plainly, hand off a concrete thing to check
each round. Commit only when asked. Don't bump pinned deps. Keep the four gates
green. Device/Apple behavior validates only on the user's hardware. Full norms +
gotchas in `docs/ARCHITECTURE.md`.
