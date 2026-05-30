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

## 1. Certificate management + credential settings UI  — **DONE 2026-05-28**

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

**Status (2026-05-28): DONE — hardware-validated.**
- Core: `signer::{list_certs, revoke_cert}` drive the fork's `cert list|revoke`
  (parsing its human output — no new fork patch); `parse_cert_list` is unit-tested.
- New error `AppleCertLimitReached` (category + remediation), classified off Apple's
  "already have a current … certificate" text (portal code 7460).
- Tauri commands `list_certificates` / `revoke_certificate`; IPC `CertInfo`.
- UI: `screens/Settings.tsx` (a new "settings" overlay; the sidebar/dashboard
  "settings" nav now opens it, not Setup) with a Certificates list+revoke and an
  Apple ID change/forget form. Plus the **auto-prompt at the cap**: when an
  install/refresh fails with `AppleCertLimitReached`, the modal shows a "Manage
  certificates" button that jumps to Settings.
- **Hardware test (2026-05-28):** see-certs, revoke-then-sign, and switch/forget
  Apple ID all passed on the user's device. The cap auto-prompt couldn't be
  force-triggered (didn't hit a 3rd-cert request) but the cap state was seen.
- **2FA gap CLOSED:** the test hit a fresh 2FA challenge in Settings, confirming
  the gap was real, so an inline prompt is now wired: `list_certs`/`revoke_cert`
  take an optional `two_fa_code` (mirrors `install`), surfaced as a `TwoFaPrompt`
  in the Certificates panel (held code reused across cert calls).
- **Follow-up UX shipped same session:** `credential_status` now returns the
  signed-in Apple ID; the Settings Apple ID section shows an identity row + a
  "Switch account" toggle instead of an always-open empty login form.

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

## 4. AUR packaging  ← start here

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

## 7. UX cleanup — workflow redundancies + dead controls

**Why:** a walkthrough of the live app (2026-05-28) found controls wired to
nothing, the same concept built two or three different ways, and one surface
that's promised in the nav but missing. None block functionality, but together
they read as half-finished and leak trust. Grouped by effort; do 7a/7b first.

### 7a. Activity view — **DONE 2026-05-29**
The `activity` sidebar nav item is a no-op, yet the backend is already live:
`get_activity_log` (`lib.rs`) reads a real `activity_log` table that **installs**
(`installs.rs`) and the **refresh scheduler** (`refresh/scheduler.rs`, severities
`info`/`warn`/`error`, ops `install`/`refresh`) already write to. For a product
whose whole point is unattended background refresh, "what happened while I was
away" is the most valuable missing screen and it's nearly free.
Shipped: `screens/Activity.tsx` lists recent `activity_log` rows (severity,
operation, message, relative time) with a sensible empty state, wired as an
`"activity"` nav overlay in `ReSideApp`. Frontend build green; **not yet
hardware-verified** (the list populates after a real install/refresh).

### 7b. Dead-control sweep — **DONE 2026-05-29**
Wired or removed every control that did nothing in the **live** app:
- `activity` nav (covered by 7a) and `apps` nav (`ReSideApp.onNavigate` ignores
  both) — wire `apps` back to the dashboard.
- Sidebar Devices "+" button (`chrome.tsx`) — no handler → wire to pairing.
- App card "More" (⋯) menu (`Dashboard.tsx`) — no handler, no menu → remove.
- "Browse examples" (`Dashboard.tsx` empty state) — `disabled` forever in live → remove.
- "Help" (?) buttons in `Settings.tsx` + `Setup.tsx` — no target → remove.
- Setup "Copy" command button (`Setup.tsx`) — no handler → wire to clipboard.
- (NB: Setup per-row "Install rules"/"Enable agent" only render in the mock
  gallery — live report items carry no `action` — so they're not live-dead.)

### 7c. Unify credential entry — **DONE 2026-05-28**
Apple-ID entry existed twice with drifting copy and different controls (ImportModal:
3 radio rows keyring/session/ask; Settings: 2 pills keyring/session). Extracted
`components/credentials.tsx` as the single source: `AppleIdFields` (email/password),
`RememberChoiceField` (tier-configurable radio rows — Import passes all three,
Settings passes `["keyring","session"]`), `ApplePasswordNote`, and `toRememberMode`
(maps the UI's "ask"/"session" → the backend's `session` tier). Both screens now
render the same component; the local `RememberOption`/`RememberPill` are gone.
Frontend build green. **Not hardware-verified** — needs a real sign-in to confirm.

### 7d. Fix the Setup overlay vs. inline check — **DONE 2026-05-28**
Resolved by **dropping the redundant overlay** (the chosen option). The detailed
Setup overlay's only live entry was the inline check's "Open detailed view"
button, and in live mode it was strictly worse (report items carry no `action`,
so its fix buttons never rendered, while the inline check wires "Enable agent").
Removed the button, the `setup` overlay branch + `Setup` import in `ReSideApp`,
and `onOpenSetup` from `GetStartedHandlers`. The Dashboard inline system check is
now the single system-status surface. `Setup.tsx` stays as a design-gallery
screen (still imported by `Gallery`), so re-adding a reachable System view later
is trivial.

### 7e. De-duplicate onboarding  (medium)
The Pairing overlay re-presents a "Setup · step 2 of 3" wizard rail that
duplicates the Dashboard `GetStartedPanel` and is misleading (it's an on-demand
overlay, not step 2 of a linear flow). Its two footer CTAs ("Skip — USB only" /
"Enable Wi-Fi refresh") both just close the overlay. Collapse to one honest
action and drop the duplicate step rail.

### 7f. Pairing auto-chain + Wi-Fi vocabulary  (polish)
Pair → re-check Dev Mode → Establish tunnel → Check Wi-Fi is four manual clicks;
auto-run the tunnel + Wi-Fi check after a successful pair when Dev Mode is on.
Collapse the three overlapping Wi-Fi entry points (passive rail poll, "Connect
over Wi-Fi" resolve, Pairing "Check Wi-Fi" reachability) into one user concept.
**Hardware feedback 2026-05-29:** the user asked whether the "Establish tunnel" /
"Check Wi-Fi" buttons do anything and whether the Devices screen is even needed.
They DO call live backend commands, but the manual chain is the clunk — fold the
"is this screen necessary / auto-chain it" question into 7e + this item.

### 7g. Persist theme — **DONE 2026-05-28**
`ReSideApp` now persists the light/dark choice to `localStorage` (`reside-theme`)
and, on first run with nothing stored, falls back to the OS
`prefers-color-scheme`. Wrapped in try/catch so a missing storage API just means
it doesn't persist that session.

### 7h. Persistent sidebar / consistent chrome — hardware feedback 2026-05-29
The Dashboard sidebar (nav + device card + agent card) vanishes when you open
Devices/Activity/Settings, because each of those is a full-screen overlay with
its own bespoke left rail — switching surfaces feels like jumping between
different apps. Make the persistent sidebar (and its active-nav highlight) stay
put across all surfaces, swapping only the main pane. This also closes the §7d
gap: once the system check is green there's currently NO way to review system
status (the inline check only shows during onboarding); a persistent "System"
entry would restore that. (User flagged this directly during §7b verification.)

### 7i. Don't offer "Connect over Wi-Fi" before pairing — **DONE 2026-05-29**
On a fresh/new-user state the Devices rail showed "An iPhone is reachable over
Wi-Fi" + a Connect button before any device was paired — but connecting/refreshing
over Wi-Fi rides on the USB-minted pairing record, so the action couldn't work yet.
Shipped: `WifiEmptyState`/`DevicesRail` now take a `paired` prop; when an iPhone is
reachable but nothing has been paired, the rail shows an informational nudge
("Plug it in over USB once to pair — then Wi-Fi refresh works on its own.") with
NO Connect button. The gate is `hasPairedDevice = hasInstalls || pair.isSuccess`:
a successful install is the persistent proof of a pairing (`installs.rs` writes the
device's `pairing_status='paired'` row), and `pair.isSuccess` covers the just-
-paired-this-session case before any install. NB: a dedicated `has_paired_device`
backend command was considered and rejected — `pairing_status='paired'` is written
ONLY on install, so it carries the same bit as `apps.length > 0`, for zero extra
Rust/IPC surface. Frontend build green; **not yet hardware-verified**. Pairs with
7f's Wi-Fi-vocabulary cleanup.

### 7j. Modals stay light in dark mode — **DONE 2026-05-29**
The install (`ImportModal`) and refresh (`RefreshModal`) dialogs rendered all-white
even with dark mode on. Root cause was the documented theming gotcha: they render
as siblings of `Dashboard`, OUTSIDE the `GnomeWindow` `data-theme` wrapper, so
their `dark:` utilities (which compile to `[data-theme=dark] .dark\:…` descendant
selectors) never matched. Fix: **hoisted a single `data-theme` onto ReSideApp's
root div** (the chosen option). It anchors the dark-variant descendant selectors
for EVERY surface — both modals and any future sibling — at one point; the per-
window `data-theme` in `GnomeWindow` is now redundant but harmless (same value,
same selector). **Follow-up (same day):** with the surfaces correctly Dracula
(verified by pixel-sampling a static harness — card `#282a36`, footer `#21222c`,
primary button purple), the one element still off-theme was the **native radio
buttons** in the credential chooser, which kept the browser's default blue accent
(`#99c8ff`). Added `accent-color: var(--dr-purple)` (dark) / `var(--ctp-mauve)`
(light) for native `input[type=radio|checkbox]` in both theme sheets, and gave the
modal close buttons a `dark:hover:text-slate-100` (they previously darkened to
near-invisible on hover). Radios now render `#bd93f9`. **Follow-up 2 (from a live
hardware screenshot):** the modal card itself was correct Dracula (`#282a36`), but
the whole app *behind* it read bluer/colder (`#191d2b`) than the warm modal — a
temperature clash the user flagged as "colors don't match." Root cause: the modal
**backdrop scrim** was bare `bg-slate-900/40`, which Dracula never remaps, so it
dimmed everything with stock cold `#0f172a`. Fixed by adding `dark:bg-slate-950/80`
to both modals' backdrop (an already-remapped token → `#21222c` at 80% in dark),
so the dim stays in the Dracula family with no blue cast; the lit modal still pops
via its lighter `#282a36` surface. Verified by reversing the scrim math on the
screenshot + re-rendering. **Follow-up 3 (live screenshot):** the "Install app"
heading and the IPA filename had **no text-color class** — they inherited. Inside
the dashboard that's fine (GnomeWindow's root sets a base `text-slate-900
dark:text-slate-100`), but the modals render OUTSIDE GnomeWindow, so their
uncolored text fell back to the browser default (near-black) instead of the
Dracula foreground. Fixed by giving each modal card the same base text color
(`text-slate-900 dark:text-slate-100` → `--dr-text #f8f8f2` in dark), which covers
the heading, filename, and any other uncolored text at once. Frontend build green;
**not yet hardware-verified**.

## 8. Certificate count accuracy — hardware feedback 2026-05-29

**Why:** on hardware, an install failed with Apple's ~2-cert cap while
Settings → Certificates listed only **one**. Revoking it unstuck the install, but
the mismatch means the UI's cert count can disagree with what Apple actually
counts — confusing and a little alarming ("did I lose a cert?").

**Likely causes:** (a) Apple's cap counts a **pending certificate request** (the
7460 text is "…or a pending certificate request"), which is never an issued cert
so `cert list` can't show it; and/or (b) `signer.rs::parse_cert_list` silently
drops any cert whose line doesn't match the exact 3-backtick shape (a back-tick
in the name, a wrapped line) — an invisible under-count.

**Refinement 2026-05-29:** the user reports Settings has *always* shown only ONE
cert, never two. That favors a **persistent** under-count (a consistently-dropped
parse line, or `cert list` omitting a cert from another machine/context) over the
transient pending-request theory. Their account is fine (revoke→reinstall leaves
exactly 1, as expected). First diagnostic step is to capture the fork's raw
`cert list` stdout for their account and diff it against what `parse_cert_list`
keeps.

**Scope:** make the listed count reconcile with the cap. Surface pending requests
(or at least explain them in the cap message), and harden `parse_cert_list`
against format variation instead of dropping rows; emit a "couldn't parse N
lines" signal so a drop is visible, not silent.

**Done when:** when signing fails at the cap, Settings shows enough to explain
*why* (issued + pending), and no issued cert is ever silently missing. Validate
on the user's account.

## Standing constraints

User is **not a developer** — explain plainly, hand off a concrete thing to check
each round. Commit only when asked. Don't bump pinned deps. Keep the four gates
green. Device/Apple behavior validates only on the user's hardware. Full norms +
gotchas in `docs/ARCHITECTURE.md`.
