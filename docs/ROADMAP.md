# ReSide Roadmap

The single, cohesive list of work for the next agent. Read `docs/ARCHITECTURE.md`
first. **Remaining work** is the part to work through, in priority order; each item
says **why**, **scope**, **where**, and **done-when**. **Completed** below is the
condensed archive (kept for the load-bearing gotchas). Do remaining items
top-down; don't opportunistically scaffold lower ones.

Section numbers (§1, §7h, …) are stable labels — they're referenced from commit
messages and notes, so they're preserved even though the items are now ordered by
priority rather than by number.

## Current state (2026-05-29)

Functionally complete and hardware-validated: sign + install over USB **and**
Wi-Fi, auto-refresh engine + unattended systemd agent, 3-tier credentials, UX
redesign, system-tray icon. **Public since 2026-05-28**: repo, the patched signer
fork (`foreverlasting/Sideloader`, branch `reside-automation`), and the
**v0.4.1 release** (`ReSide-0.4.1-linux-x86_64.tar.gz`, sha256
`6312a2dfa81029b0f220235f7f984efc798e58e2bf54a6231875e1934f70bf57`).

**Merged:** PR #16 (`ux-modals-wifi-gate` → main) — §7i + §7j. PR #17
(`ux-persistent-sidebar` → the `ux-modals-wifi-gate` integration branch) — §7h.
**Main does not yet carry §7h:** it lives on `ux-modals-wifi-gate` (`ad8f3a6`),
waiting on that branch's next merge up. Both upstream merges were squashes, so
local feature branches no longer share clean ancestry — stack new work on the
integration branch, not on the orphaned per-feature branches.

**In flight:** §7e + §7f (`ux-devices-pane`, stacked on `ux-modals-wifi-gate`) —
Devices becomes an in-shell pane; build green, not yet hardware-verified.

**Hardware-verified 2026-05-30:** §7e + §7f (Devices pane). §7h (persistent
sidebar) is verified by extension — the Devices pane runs *inside* the persistent
shell, so navigating it on hardware exercises §7h.

**Pending hardware verification** (built + green, not yet confirmed on the user's
device): §7a Activity view.

---

# Remaining work

## §7h. Persistent sidebar / consistent chrome — MERGED 2026-05-30 (PR #17 → `ux-modals-wifi-gate`; not yet on main; hardware-verified via §7e/§7f)

**Why:** hardware feedback 2026-05-29 (user flagged directly during §7b verify).
The Dashboard sidebar (nav + device card + agent card) vanishes when you open
Devices/Activity/Settings, because each of those is a full-screen overlay with
its own bespoke left rail — switching surfaces feels like jumping between
different apps.

**Scope:** make the persistent sidebar (and its active-nav highlight) stay put
across all surfaces, swapping only the main pane. This also closes the §7d gap:
once the system check is green there's currently NO way to review system status
(the inline check only shows during onboarding); a persistent **System** entry
would restore that.

**Done when:** sidebar + active-nav highlight persist across Dashboard / Devices /
Activity / Settings / System; only the main pane swaps. Validate on hardware.

**Implementation plan (chosen scope: persistent sidebar + System view; Pairing
stays an on-demand overlay — the Devices/Pairing rework is left to §7f):**

*Target:* one `GnomeWindow` + one `Sidebar` rendered once in `ReSideApp`; a
`surface` state (`"apps" | "activity" | "settings" | "system"`) picks the main
pane. Pairing is a transient overlay layered on top. Install/refresh modals stay
siblings under the root `data-theme` node — §7j is untouched. No Rust changes;
frontend-only.

Files & changes, in order:
1. **`components/chrome.tsx`** — add a **System** nav item (icon `shieldCheck`);
   stop hardcoding `Sidebar.active` (caller passes the live surface so the
   highlight tracks it); extract an `AppShell` (GnomeWindow + Sidebar +
   `<main>{children}</main>`) so screens stop re-implementing the scaffold.
2. **`ReSideApp.tsx`** — replace the `Overlay` type with a `surface` state
   (default `"apps"`); Pairing becomes its own `pairingOpen` overlay. Render
   `AppShell` once with the live device/agent/Wi-Fi props (lifted up from
   Dashboard), then `switch(surface)` for the pane. `onNavigate`:
   apps/activity/settings/system set `surface`; devices opens the Pairing overlay.
   Drop the per-screen `toolbarExtra`/`onClose`/"Done" plumbing.
3. **`screens/Dashboard.tsx` → Apps pane** — remove its own `GnomeWindow` +
   `Sidebar`; keep the `<main>` content (headline, Get-Started panel, app grid).
4. **`screens/Settings.tsx` → Settings pane** — remove `GnomeWindow` + the
   bespoke 260px rail + "Done" footer; keep Certificates + Apple ID sections.
   Drop `railExtra` (sidebar already shows the device/Wi-Fi card); relocate or
   drop the two tip cards.
5. **`screens/Activity.tsx` → Activity pane** — same surgery; keep header + list.
6. **New `screens/System.tsx`** — renders the setup-check report (backend/tunnel
   pills + dependency rows + "Run check"), reusing `InlineSystemCheck`/
   `InlineCheckRow` extracted out of `Dashboard.tsx`, wired to the `setup` query
   already in `ReSideApp`. Closes the §7d gap.
7. **`Gallery.tsx`** — wrap the now-pane-only mock screens in `AppShell` so the
   full-window previews still render.

Work order: chrome → ReSideApp → Apps pane → Settings pane → Activity pane →
System pane → Gallery fixups → `pnpm` build/typecheck gate → hardware verify.

Risks: (1) **Gallery coupling** — `Dashboard`/`Settings`/`Setup` double as
full-window mock screens; they must be re-wrapped in `AppShell` in gallery mode.
(2) **Pairing inconsistency** — Devices nav pops an overlay while others swap
panes; an intentional gap parked for §7f. (3) **Lifting device/agent data** up to
the shell so every surface shows identical live state (ReSideApp already owns the
queries). (4) Minor per-surface subtitle/tip-card copy.

**What shipped (branch `ux-persistent-sidebar`):** the plan held, with three
deviations that made it *simpler*:
- **No `AppShell` wrapper.** Since `Gallery` only renders `Dashboard` (not
  Settings/Activity), the cleanest single-instance persistence was to keep
  `Dashboard` as the live shell and keep it mounted. It gained `active`,
  `mainContent`, and `subtitleOverride` props; ReSideApp renders ONE `<Dashboard
  live>` always and swaps `mainContent` (— → Apps; `<Settings/>` / `<Activity/>` /
  `<System/>`). The sidebar, window chrome, and toolbar never unmount. So risk (1)
  evaporated — Gallery needed **no** changes.
- **Settings & Activity became panes** (stripped their `GnomeWindow` + bespoke
  rail + "Done" footer; nav is the way out). The Settings keyring note was
  relocated into the pane body; the rail tip cards were dropped.
- **New `screens/System.tsx`** renders the dependency check standalone (reuses
  `InlineSystemCheck`, now exported with an `inset` prop), wired to the existing
  `setup` query via the hoisted `getStarted` handlers. Sidebar gained a **System**
  nav item (`shieldCheck`). Closes the §7d gap.
- **Dropped the multi-device `DevicesRail` + per-device selection** (`selectedUdid`)
  — it only ever lived in the old Settings rail; `target` is now the first detected
  device, and choosing among several plugged-in devices is folded into the §7f
  Devices-surface work.

ReSideApp now tracks a `surface` state (`apps|activity|settings|system`) plus a
separate `pairingOpen` overlay (Devices nav). Frontend build (`tsc --noEmit &&
vite build`) is green; no Rust touched. **Left to do: hardware verify** — click
Apps → Activity → System → Settings and confirm the sidebar + highlight stay put
and only the pane swaps; confirm the new System view shows the dep check.

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

## §7f. Devices surface + Wi-Fi vocabulary — DONE + HARDWARE-VERIFIED 2026-05-30 (`ux-devices-pane`, PR #18; auto-chain deferred)

**Why:** Pair → re-check Dev Mode → Establish tunnel → Check Wi-Fi was four manual
clicks across a full-screen overlay; hardware feedback 2026-05-29 questioned whether
the buttons did anything and whether the Devices screen was even needed. They DO call
live backend commands — the manual chain was the clunk.

**What shipped:** "Devices" is now a first-class in-shell pane (`screens/Devices.tsx`)
rendered through the persistent shell's `mainContent` (like System/Activity/Settings),
not a takeover — answering "is this screen needed?" with a real device *manager*.
Single-device-first: a switcher row appears only when >1 device. A **connection ladder**
(Paired → Developer Mode → Secure tunnel → Wi-Fi refresh) replaces the scattered
panels, with downstream rungs `locked` behind the current blocker so exactly one
action is live; warn/error/remediation copy is ported from the old
`DevModeGate`/`TunnelPanel`/`WifiPanel`. The three Wi-Fi entry points now read as one
concept: the ladder's Wi-Fi rung for a connected device, the sidebar `WifiEmptyState`
+ cold-start nudge for an unpaired one. `selectedUdid` now drives `target`, so the
per-device queries re-scope — the multi-device selection §7h deferred has a home.
Developer Mode is gated on the *standing* paired state, not the transient pair phase.
No "Forget device" control (there's no backend unpair — §7b dead-control rule).
Design artboards in `docs/artboards/devices-pane*.html`.

**Deferred (still open):** true **auto-chain** — auto-running the tunnel + Wi-Fi check
after a successful pair when Dev Mode is on. The ladder makes the manual chain legible
(one live action at a time) but still requires the clicks. Multi-device readiness also
still rides the install-coupled `pairing_status` (§7i) — exact for one device,
approximate for several; a per-device paired signal is the real fix.

## §7e. De-duplicate onboarding — DONE + HARDWARE-VERIFIED 2026-05-30 (`ux-devices-pane`, PR #18)

**Why:** the Pairing overlay re-presented a "Setup · step 2 of 3" wizard rail that
duplicated the Dashboard `GetStartedPanel` and was misleading (an on-demand overlay,
not step 2 of a linear flow); its two footer CTAs ("Skip — USB only" / "Enable Wi-Fi
refresh") both just closed it.

**What shipped:** pairing is no longer a full-screen overlay at all. The transient
trust handshake became a focused modal (`screens/PairModal.tsx`, the ImportModal
pattern) with a single honest action — no fake wizard rail, no dual closing CTAs.
Everything *after* the handshake (Dev Mode / tunnel / Wi-Fi) moved to the §7f Devices
pane's ladder. The gallery-only artboard (`screens/Pairing.tsx`) is untouched.

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

## Standing constraints

User is **not a developer** — explain plainly, hand off a concrete thing to check
each round. Commit only when asked. Don't bump pinned deps. Keep the four gates
green. Device/Apple behavior validates only on the user's hardware. Full norms +
gotchas in `docs/ARCHITECTURE.md`.
