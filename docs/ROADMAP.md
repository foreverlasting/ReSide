# ReSide Roadmap

Prioritized work for the next agent. Read `docs/ARCHITECTURE.md` first. Each item
says **why**, **scope**, **where**, and **done-when**. Do them top-down; don't
opportunistically scaffold lower items.

## Current state (2026-05-26)

Functionally complete and hardware-validated: sign + install over USB **and**
Wi-Fi, auto-refresh engine + unattended systemd agent, 3-tier credentials, UX
redesign. The first GitHub Release is **staged**: private repo
`foreverlasting/ReSide` (branch `automation-layer` pushed as `main`), a **draft**
release `v0.4.0` with the 20 MB tarball attached. **Not yet public.**

Go-live is the user's call (all outward-facing): flip repo public → publish the
Sideloader fork public (GPL source obligation; commands in `packaging/RELEASING.md`)
→ un-draft the release. Do not do these without explicit go-ahead.

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

## 2. Pre-public polish (cheap; do before strangers arrive)

- **`mdns_sd` ERROR log noise** — a benign shutdown race logs at ERROR. Downgrade
  to debug/warn in `transport/mdns_discovery.rs` so logs don't cry wolf.
- **Mock gallery screens** — `Gallery.tsx`/`Import.tsx`/`Install.tsx`/`Tray.tsx`
  render placeholder data (browser-only via `isTauri()`). Decide: label as
  previews or strip. Shipping fake data in a public repo reads as half-built.
- **Code-doc pass for live-vs-parked** — `lib.rs` and `signing/mod.rs` docs were
  corrected as examples; finish marking the rest of `signing/*` and
  `setup/adi_provision.rs` as PARKED/superseded so the next reader isn't misled.
- **iOS minimum claim** — README says 17.4+; verify or soften before users test
  on older devices.
- **Stale `icons/icon.ico`** — old icon, Windows-only, harmless on Linux; replace
  or drop the loose end.

## 3. Wi-Fi devices in the Devices rail

**Why:** Wi-Fi is the headline feature, but the Devices rail lists USB only — the
thing you're proudest of is invisible. Analysis + approaches A/B/C/D were drafted
earlier (the gap was deferred, nothing built).

**Scope/where:** surface mDNS-discovered Wi-Fi devices (`transport/mdns_discovery.rs`)
into the device list the front end renders. **Done when:** a paired device on
Wi-Fi shows in the rail with correct status.

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
