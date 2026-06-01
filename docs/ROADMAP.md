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
(§7i/§7j), and #19 (§7h + §7e + §7f, squash `2368894`). All merged `ux-*` feature
branches are deleted. **§7k + the multi-device / Wi-Fi-ladder work landed on main
2026-05-31** (PR #27, squash `40dc942`) — per-device readiness, multi-device
correctness, and a remodeled Wi-Fi ladder; hardware-verified (Wi-Fi refresh
confirmed working end-to-end). See the §7k Completed entry.

**Latest release: v0.5.1** (2026-05-30, tag `v0.5.1`) — a patch over v0.5.0
carrying the §8 certificate work (clearer cap message + parser hardening). It was
the **first release built and published automatically by the §5 CI workflow** from
a pushed tag; the v0.5.0 release (tag `2a51f40`) shipped the full §7 UX line over
v0.4.1. §7a Activity view is the one piece only observed populating (no explicit
hardware sign-off).

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

**Progress 2026-05-30 — parse-hardening landed AND root-cause diagnosed on
hardware:**

*Parse-hardening (code, committed-pending):* `signer.rs::parse_cert_list` no
longer silently drops rows. It returns `ParsedCertList { certs, unparsed }`: a
line is a cert row only if it carries BOTH the "serial number" **and** "machine
named" anchors (the old single-"serial number" filter let prose noise through and
leaned on the field-count mismatch to reject it), and a both-anchors row that
won't resolve to the three back-tick fields is retained as `unparsed` instead of
discarded. `list_certs` logs a `tracing::warn!` ("cert list: N line(s) looked like
a certificate but did not parse…") when that set is non-empty. Unit test
`parse_cert_list_records_unparsable_rows_instead_of_dropping` covers it.

*Hardware diagnostic (phone connected, dev build + temporary raw-stdout dump,
since reverted):* the fork's raw `cert list` for the user's account reports
**`You have 1 certificates registered.`** with exactly one cert
(`iOS Development: Eric Marshall`, serial `17FF…D756`, machine `Sideloader`) — and
it **parses cleanly** (no `unparsed` warning fired). So the "Settings shows 1 cert"
display is **correct**; it was never a parser under-count. That **rules out theory
(b)** (a dropped/omitted issued cert) and points squarely at the **pending
certificate request** (theory (a)): Apple's cap counts "a current cert **or** a
pending request," and a pending request never appears in `cert list`, so the UI
legitimately can't list it. Revoke-→-reinstall worked because it freed the one
issued slot.

*Cap message (done):* `error.rs`'s `AppleCertLimitReached` copy now reads —
"Apple caps a free account at ~2 signing certificates, and counts any pending
request — which won't show in the list, so this can trigger even when Settings
shows just one. Revoke the certificate in Settings → Certificates to free a slot,
then try again." (The old copy said "revoke an old one," which read as nonsense
when the list shows exactly one.)

**§8 is functionally complete:** no issued cert can be silently dropped (hardened +
hardware-verified the real line parses), and the cap failure now explains *why* it
can fire with one cert shown. Residual is opportunistic only — confirm the new cap
copy on a real cap hit if one occurs naturally (couldn't be force-triggered in §1
testing either). The parse-hardening stays as defensive insurance; plumbing the
unparsed-count to the UI is deferred unless the log signal ever proves insufficient.

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

**DONE + live-validated 2026-05-30.**
`.github/workflows/release.yml` triggers on `v*`, builds on `archlinux:latest`,
re-runs the gates, and runs `build-tarball.sh`, attaching the tarball + `.sha256`
to the tag's GitHub Release. The hard part (the prebuilt **D** `sideloader`) is
solved with the **prebuilt-helper cache** option: both helpers live as assets on a
dedicated `helpers-v1` prerelease that the workflow downloads (`gh release
download`), so CI never rebuilds the ldc fork. A tag/version mismatch fails the job
fast; an already-drafted release for the tag gets the assets uploaded onto it.
RELEASING.md documents the tag-to-ship flow and how to refresh `helpers-v1`. The
manual path is untouched (the §5 "don't block manual release" constraint).
**Proven live:** the `v0.5.1` tag ran end-to-end (workflow succeeded; tarball +
`.sha256` published and `sha256sum -c`-verified) with no manual packaging.

## §6. Upstream the TLS-verify fix to Dadoum

**Why:** it's a real security fix; acceptance shrinks the fork to one patch (the
non-interactive login) and is good open-source citizenship.

**Scope:** open a PR to `Dadoum/Sideloader` from `673db69`.

**Done when:** the PR is open upstream.

## §7l. Retire (or repurpose) the titlebar "Tunnel" pill

**Why:** the §7k remodel established that the in-process RSD tunnel is *not* on the
install/refresh path — nothing establishes one anymore — so the titlebar pill
(`get_tunnel_status` → `any_connected`) now reads permanently "No tunnel." A
forever-off indicator is noise that implies something's broken when it isn't.

**Scope:** hide the pill (keep the `TunnelManager`/`get_tunnel_status` backend infra
dormant per the §7k decision), OR repurpose it into a signal that reflects real
state (e.g. selected-device reachability USB/Wi-Fi). Smallest honest fix is to drop
the pill from the titlebar in `ReSideApp.tsx`/`chrome.tsx`.

**Done when:** the titlebar no longer shows a permanently-off "Tunnel" indicator;
the tunnel backend stays compiled and callable (not deleted).

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
- **§7k. Pairing auto-chain + per-device paired signal** — DONE 2026-05-31, on main
  (PR #27, squash `40dc942`), hardware-verified. Three parts:
  (1) **Per-device paired signal** — `DeviceInfo.paired` reads straight from the
  usbmuxd trust store (`annotate_paired` → `get_pair_record` per UDID); the ladder's
  `selectedPaired` keys off the *selected* device (OR'd with this session's
  `pair.isSuccess` for the just-paired UDID), replacing the global `hasInstalls` bit.
  The old global `hasEverPaired` survives only for the no-device surfaces (cold-start
  Wi-Fi nudge, §7i). (2) **Transport-aware tunnel** — `TunnelStatus` gained a
  `transport` field; new `tunnel_status_for(udid)` (per-device, distinct from the
  aggregate titlebar pill) and `establish_tunnel_for_transport(udid, wifi)` commands;
  a USB tunnel left behind after the device moves to Wi-Fi reads as stale (transport
  mismatch) so the ladder re-prompts. (3) **Auto-chain** — after a successful pair,
  once Developer Mode reads on, a `useRef`-guarded effect walks the tunnel + Wi-Fi
  rungs once each (no auto-retry; manual Retry stays the recovery path), scoped to
  the just-paired & selected UDID, never on cold launch.
  **Scoped cut:** Wi-Fi RSD tunnel *establishment* (idevice `remote_pairing`) stays
  the deferred "later slice" `remote_xpc` already calls out — `connect(udid, wifi=true)`
  fails fast with the new `WifiTunnelUnsupported` error ("connect via USB") rather than
  faking a tunnel. A fresh pair is always USB, so the auto-chain exercises the live USB
  path; the Wi-Fi establish path is wired end-to-end but intentionally errors until that
  slice lands.
  **Hardware verify — multi-device (iPad + iPhone):** pair one, let its ladder
  auto-advance, then pair the other; confirm each device's ladder tracks
  *independently* when you switch in the device switcher (Paired / Dev Mode / tunnel
  per-UDID), and that **both tunnels stay live at once** (switching to the iPad shows
  its tunnel still up while the iPhone's is too — `TunnelManager` is a per-UDID
  HashMap). Known-correct-by-design, but unverified on hardware: (a) the **auto-chain
  runs for one device at a time** — the most-recently-paired *and* selected UDID, since
  it rides the single `pair` handle; pairing B before A's chain finishes abandons A's
  auto-walk (A still shows correct *status*; its last rung is a manual click). This
  matches the inherently sequential pair flow (one Trust tap each). (b) the **Wi-Fi
  reachability rung is still network-wide** (`checkWifiAvailability`, not device-scoped
  — pre-existing §7f/§7i limitation), so that one rung reads identically on both
  ladders.
  **Apps-grid device attribution (found on hardware 2026-05-31):** with the same app
  on two devices the grid showed two cards with no owner, and the header
  mis-attributed the global install count to the *selected* device ("2 apps on Eric's
  iPad" when one was on the iPhone). Fix: `list_apps` now joins `devices.name` (rides
  on each `InstalledApp` as `deviceName`, so it groups even when the device is
  offline); `LiveApps` groups cards under a per-device subheader; the header counts
  *distinct* devices ("N apps across M devices", or "N apps on <name>" for one).
  **Wi-Fi device identity mislabel (found on hardware 2026-05-31):** an iPhone +
  iPad over Wi-Fi *both* rendered as the iPad (same name/model/iOS) — the per-device
  lockdown read over netmuxd returns the wrong device's identity. The DB has each
  paired device's true identity (captured over USB at install), so the `list_devices`
  command now overrides name/ios_version/product_type from the `devices` table for
  **Wi-Fi rows** (USB rows keep their reliable live read). Added a `product_type`
  column (migration `0002`) persisted on install/refresh; existing rows have it null
  until their next USB install, so model shows "—" meanwhile (better than confidently
  wrong). Root cause (netmuxd lockdown misrouting) is upstream and only bites the
  deferred Wi-Fi-tunnel ops; this fixes the *display*. Auto-refresh enable in dev also
  needed `cargo build -p reside-agent` (binary absent in `tauri dev`) — env gap, not a
  code bug, and unrelated to device count.
  **Ladder remodel — the "Secure tunnel" rung was a false gate (found on hardware
  2026-05-31, SUPERSEDES parts (2)+(3) above):** hardware-verified that **Wi-Fi
  refresh works end-to-end** (an iPad refresh over Wi-Fi bumped its expiry + logged
  clean). Tracing showed install/refresh route through the **external signer over
  netmuxd** (`route_to` → `USBMUXD_SOCKET_ADDRESS`; the signer does its own iOS-17.4
  tunneling) and **never** touch ReSide's in-process RSD tunnel (`TunnelManager` has
  zero non-UI callers). So the ladder's "Secure tunnel" rung gated nothing real and,
  because it can't go green over Wi-Fi, *wedged the whole Wi-Fi ladder* (also: the
  Developer-Mode read was USB-only). **Remodel:** ladder is now **Paired → Developer
  Mode → Ready to refresh** (3 rungs; tunnel + Wi-Fi-scan rungs dropped). Developer
  Mode over Wi-Fi reads a value cached from USB reads / installs (migration `0003`
  adds `developer_mode_checked_ts`, backfilled from existing installs since an install
  proves Dev Mode was on; `developer_mode_status` command falls back to the cache when
  the fast USB read fails). The §7k auto-chain + per-device tunnel UI wiring were
  removed; the per-device **paired signal** (part 1) and the in-process RSD tunnel
  **infra** (`TunnelManager`, `establish_tunnel*`, titlebar pill) stay (dormant, kept
  for future in-process RemoteXPC work — user's call). *Loose end → §7l:* the titlebar
  "Tunnel" pill now reads permanently "No tunnel" (nothing establishes one). Also
  fixed this pass: sidebar lists **all** located devices
  (selectable), and the Model field is hidden when unknown rather than showing "—".

## Standing constraints

User is **not a developer** — explain plainly, hand off a concrete thing to check
each round. Commit only when asked. Don't bump pinned deps. Keep the four gates
green. Device/Apple behavior validates only on the user's hardware. Full norms +
gotchas in `docs/ARCHITECTURE.md`.

## Lessons learned (2026-05-31, §7k / multi-device work)

- **Verify gates by a command's *own* exit code — never through a pipe.** `cargo fmt
  --all -- --check | tail` (and `gh run watch … | tail`) report the *pipe's* exit
  (tail's `0`), masking a real failure. This produced a false-green local fmt check
  and a red CI run (PR #27, first attempt). Run gate commands unpiped, or check the
  right element's status; treat any "clean" result that also printed diff lines as a
  failure.
- **The in-process RSD tunnel is NOT on the install/refresh path.** Install/refresh
  route through the external signer over netmuxd (`route_to` → `USBMUXD_SOCKET_ADDRESS`;
  the signer does its own iOS-17.4 tunneling). `TunnelManager`/`establish_tunnel*` have
  zero non-UI callers. Don't gate refresh-readiness on ReSide's own tunnel, and don't
  rebuild a feature assuming it's load-bearing — it isn't (yet). (Drove the §7k ladder
  remodel; see §7l for the leftover pill.)
- **Confirm a capability on hardware before redesigning UI around it.** Verifying Wi-Fi
  refresh actually worked (DB expiry delta + activity row) *before* remodeling the
  ladder turned an assumption into a fact and avoided building the wrong thing.
- **Over Wi-Fi, trust persisted (USB-captured) DB identity over live netmuxd reads** —
  the netmuxd lockdown read can return the wrong device's name/version/model.
