# ReSide Roadmap

The single, cohesive list of work for the next agent. Read `docs/ARCHITECTURE.md`
first. **Remaining work** is the part to work through, in priority order; each item
says **why**, **scope**, **where**, and **done-when**. **Completed** below is the
condensed archive (kept for the load-bearing gotchas). Do remaining items
top-down; don't opportunistically scaffold lower ones.

Section numbers (§1, §7h, …) are stable labels — they're referenced from commit
messages and notes, so they're preserved even though the items are now ordered by
priority rather than by number.

## Current state (2026-05-30)

Functionally complete and hardware-validated: sign + install over USB **and**
Wi-Fi, auto-refresh engine + unattended systemd agent, 3-tier credentials, UX
redesign, system-tray icon. **Public since 2026-05-28**: repo, the patched signer
fork (`foreverlasting/Sideloader`, branch `reside-automation`), and the
**v0.4.1 release** (`ReSide-0.4.1-linux-x86_64.tar.gz`, sha256
`6312a2dfa81029b0f220235f7f984efc798e58e2bf54a6231875e1934f70bf57`).

**The entire §7 UX line is now on `main` and hardware-verified** — persistent
sidebar (§7h), System view (§7d gap closed), Activity view (§7a), and the Devices
in-shell pane + trust modal (§7e/§7f). It landed across PR #15 (§7a–g), #16
(§7i/§7j), and #19 (§7h + §7e + §7f, squash `2368894`). All `ux-*` feature
branches are merged + deleted.

**`main` is unreleased, ahead of v0.4.1** — next step is a release cut: version
bump to **0.5.0**, then the §5 tarball. §7a Activity view is the one piece only
observed populating (no explicit hardware sign-off).

---

# Remaining work

## §8. Certificate count accuracy

**Why:** hardware feedback 2026-05-29 — an install failed with Apple's ~2-cert cap
while Settings → Certificates listed only **one**. Revoking it unstuck the install,
but the mismatch means the UI's cert count can disagree with what Apple counts —
confusing and a little alarming ("did I lose a cert?").

**Likely causes:** (a) Apple's cap counts a **pending certificate request** (the
7460 text is "…or a pending certificate request"), never an issued cert so
`cert list` can't show it; and/or (b) `signer.rs::parse_cert_list` silently drops
any cert whose line doesn't match the exact 3-backtick shape (a backtick in the
name, a wrapped line) — an invisible under-count.

**Refinement 2026-05-29:** the user reports Settings has *always* shown only ONE
cert, never two. That favors a **persistent** under-count (a consistently-dropped
parse line, or `cert list` omitting a cert from another machine/context) over the
transient pending-request theory. Their account is fine (revoke→reinstall leaves
exactly 1). First diagnostic step is to capture the fork's raw `cert list` stdout
for their account and diff it against what `parse_cert_list` keeps.

**Scope:** make the listed count reconcile with the cap. Surface pending requests
(or at least explain them in the cap message), and harden `parse_cert_list`
against format variation instead of dropping rows; emit a "couldn't parse N lines"
signal so a drop is visible, not silent.

**Done when:** when signing fails at the cap, Settings shows enough to explain
*why* (issued + pending), and no issued cert is ever silently missing. Validate on
the user's account.

## §4. AUR packaging

**Why:** the planned real distribution channel for Arch/CachyOS users; sources
from the GitHub Release.

**Scope:** a `PKGBUILD` that pulls the release tarball (or builds from source,
incl. the fork's pinned-ldc build).

**Done when:** `makepkg -si` installs a working ReSide on a clean Arch box. (User
is on CachyOS, so this can be validated locally.) Sequence after the release is
public — it is.

## §5. Release automation (CI builds the tarball on tag)

**Why:** releases are hand-built today; automation makes them reproducible.

**Scope:** a GitHub Action that runs the gates and `packaging/build-tarball.sh` on
a `v*` tag, attaching the artifact. **Caveat:** the D signer's pinned-toolchain
build is the genuinely hard part to reproduce in CI — may need a prebuilt-helper
cache or a container image.

**Done when:** pushing a `v*` tag produces the attached tarball via CI. Don't block
any manual release on this.

## §6. Upstream the TLS-verify fix to Dadoum

**Why:** it's a real security fix; acceptance shrinks the fork to one patch (the
non-interactive login) and is good open-source citizenship.

**Scope:** open a PR to `Dadoum/Sideloader` from `673db69`.

**Done when:** the PR is open upstream.

## §7k. Pairing auto-chain + per-device paired signal — polish (deferred from §7f)

**Why:** the §7f connection ladder makes the manual chain legible (one live action
at a time) but a successful pair still needs manual clicks through Dev Mode →
tunnel → Wi-Fi. And multi-device readiness rides the install-coupled
`pairing_status` (§7i) — exact for one device, approximate for several.

**Scope:** auto-run the tunnel + Wi-Fi check after a successful pair when Dev Mode
is on (the ladder's later rungs advance on their own). Add a per-device paired
signal so the ladder reflects the *selected* device, not the global `hasInstalls`
bit.

**Done when:** a successful pair auto-advances the ladder to a Wi-Fi-ready state
with no extra clicks, and the ladder's readiness is per-device. Validate on hardware.

---

# Completed

Condensed; load-bearing gotchas retained.

- **§1. Certificate management + credential settings UI** — DONE 2026-05-28,
  hardware-validated. `signer::{list_certs,revoke_cert}` drive the fork's
  `cert list|revoke` (parsing human output, no new fork patch); `parse_cert_list`
  unit-tested. New error `AppleCertLimitReached` (portal code 7460). Tauri
  `list_certificates`/`revoke_certificate`; `screens/Settings.tsx` lists+revokes
  certs and changes/forgets Apple ID, with cap auto-prompt and inline 2FA
  (`two_fa_code` on the cert calls). `credential_status` returns the signed-in
  Apple ID. (Open under §8: the listed count can disagree with Apple's cap.)
- **§2. Pre-public polish** — DONE 2026-05-28. Killed `mdns_sd` error log noise;
  `App.tsx` gates `<Gallery/>` to non-Tauri runs; `signing/` + `setup/adi_provision.rs`
  carry `⚠️ PARKED` headers pointing at `signer.rs`; README softened to
  "Recommended: iOS 17.4+"; stale `icons/icon.ico` removed.
- **§3. Wi-Fi devices in the Devices rail** — DONE 2026-05-28, hardware-validated.
  Hybrid: passive mDNS poll surfaces a "reachable over Wi-Fi" banner; "Connect over
  Wi-Fi" spins netmuxd up on demand (`transport::muxer::resolve_wifi_devices`),
  reads name + iOS version, caches in session-only `transport::wifi_cache`, tears
  netmuxd down; `device::list_devices` merges USB ∪ Wi-Fi (USB wins on UDID).
  **Gotcha — don't regress:** Linux `usbmuxd` is udev-activated and *exits* with no
  cable, so `list_devices` treats a connection failure as "no USB devices," not
  fatal, letting the Wi-Fi cache still surface a card.
- **§7a. Activity view** — DONE 2026-05-29 (pending hardware verify).
  `screens/Activity.tsx` lists `activity_log` rows (severity/op/message/relative
  time) with an empty state, wired as an `"activity"` nav overlay. The table is
  already written by installs (`installs.rs`) and the refresh scheduler.
- **§7b. Dead-control sweep** — DONE 2026-05-29. Wired or removed every no-op live
  control: `apps` nav → dashboard; sidebar Devices "+" → pairing; Setup "Copy" →
  clipboard; removed App-card "More" menu, "Browse examples", and Help (?) buttons.
- **§7c. Unify credential entry** — DONE 2026-05-28 (needs a real sign-in to
  confirm). `components/credentials.tsx` is the single source (`AppleIdFields`,
  `RememberChoiceField`, `ApplePasswordNote`, `toRememberMode`); both ImportModal
  and Settings render it.
- **§7d. Setup overlay vs inline check** — DONE 2026-05-28. Dropped the redundant
  detailed Setup overlay; the Dashboard inline system check is the single
  system-status surface. (`Setup.tsx` stays as a gallery screen.) NB: §7h restores
  a reachable persistent **System** view.
- **§7g. Persist theme** — DONE 2026-05-28. `ReSideApp` persists light/dark to
  `localStorage` (`reside-theme`); first-run falls back to OS `prefers-color-scheme`;
  wrapped in try/catch.
- **§7i. Don't offer "Connect over Wi-Fi" before pairing** — DONE 2026-05-29 (PR #16,
  pending hardware verify). `WifiEmptyState`/`DevicesRail` take a `paired` prop; an
  unpaired-but-reachable iPhone shows a "plug in over USB once to pair" nudge with
  NO Connect button. Gate is `hasPairedDevice = hasInstalls || pair.isSuccess` (a
  successful install writes `pairing_status='paired'`, so it carries the same bit as
  `apps.length > 0` — no extra backend command needed).
- **§7j. Modals stay light in dark mode** — DONE 2026-05-29 (PR #16, pending
  hardware verify). **Theming gotcha (load-bearing for any future modal):**
  ImportModal/RefreshModal render as siblings of `Dashboard`, OUTSIDE the
  `GnomeWindow` `data-theme` wrapper, so their `dark:` utilities (which compile to
  `[data-theme=dark] .dark\:…` descendant selectors) never matched. Fix: hoisted a
  single `data-theme` onto ReSideApp's root div (anchors dark-variant selectors for
  every surface). Plus: `accent-color` on native radios/checkboxes (were stock
  blue); `dark:bg-slate-950/80` on the modal backdrop scrim (bare `slate-900/40`
  isn't Dracula-remapped → cold cast); and base `text-slate-900 dark:text-slate-100`
  on each modal card (uncolored text fell back to near-black outside GnomeWindow).
- **§7h. Persistent sidebar / consistent chrome** — DONE 2026-05-30, hardware-verified
  (via §7e/§7f), on main (PR #19). One `<Dashboard live>` stays mounted as the shell;
  a `surface` state (`apps|devices|activity|settings|system`) swaps only `mainContent`
  while sidebar/chrome/toolbar persist (gained `active`/`mainContent`/`subtitleOverride`
  props — no `AppShell`, since Gallery only renders Dashboard). Settings & Activity
  became panes (dropped GnomeWindow + bespoke rail + "Done" footer). New
  `screens/System.tsx` renders the dep check standalone (reuses `InlineSystemCheck`,
  exported with an `inset` prop) — closes the §7d gap; sidebar gained a **System** nav item.
- **§7e. De-duplicate onboarding** — DONE 2026-05-30, hardware-verified, on main (PR #19).
  Pairing is no longer a full-screen overlay: the trust handshake became a focused modal
  (`screens/PairModal.tsx`, ImportModal pattern) with one honest action — the fake
  "Setup · step 2 of 3" wizard rail and the dual closing CTAs are gone. Post-handshake
  steps moved to the §7f ladder. `screens/Pairing.tsx` kept as the gallery-only artboard.
- **§7f. Devices surface + Wi-Fi vocabulary** — DONE 2026-05-30, hardware-verified, on
  main (PR #19). "Devices" is a first-class in-shell pane (`screens/Devices.tsx`) via the
  shell's `mainContent`, not a takeover. Single-device-first (switcher only when >1
  device); a **connection ladder** (Paired → Developer Mode → Secure tunnel → Wi-Fi
  refresh) with downstream rungs `locked` behind the current blocker; warn/error copy
  ported from the old DevModeGate/TunnelPanel/WifiPanel. `selectedUdid` drives `target`
  so per-device queries re-scope. Developer Mode gated on the STANDING paired state, not
  the transient pair phase. No "Forget" control (no backend unpair — §7b rule). Artboards
  in `docs/artboards/devices-pane*.html`. **Deferred → §7k.**

## Standing constraints

User is **not a developer** — explain plainly, hand off a concrete thing to check
each round. Commit only when asked. Don't bump pinned deps. Keep the four gates
green. Device/Apple behavior validates only on the user's hardware. Full norms +
gotchas in `docs/ARCHITECTURE.md`.
