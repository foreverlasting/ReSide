# Agent Handoff Prompt

Paste the block below to kick off the next agent. It is intentionally short —
the durable detail lives in `docs/ARCHITECTURE.md` and `docs/ROADMAP.md`, which
the agent should read first.

---

You're picking up **ReSide**: a Linux-first desktop app (Tauri 2 + React/TS front
end, Rust `reside-core` back end) that signs, sideloads, and **auto-refreshes**
iOS apps by **driving a forked Dadoum Sideloader CLI** — it does not reimplement
Apple's signing stack. Solo dev, open-source GPL-3.0, Arch/CachyOS-first.

**Read first, in order:**
1. `docs/ARCHITECTURE.md` — how it works now. Critically: the **live vs parked**
   map. The live signing path is `signer.rs` (drives the fork); the `signing/`
   module + `setup/adi_provision.rs` are the **abandoned native attempt** — don't
   build on them, even though their doc comments still say "Phase 2/future."
2. `docs/ROADMAP.md` — prioritized work. Items 1–3 are **done** (cert-management +
   settings UI shipped and hardware-validated 2026-05-28, incl. inline 2FA in
   Settings). **Start at item 4: AUR packaging.**
3. Code + `git log` are ground truth; trust them over older prose. `plan.md` and
   `Product-Brief.md` are historical (pre-pivot) — `docs/ARCHITECTURE.md` supersedes them.

**State right now:** functionally complete, hardware-validated, **public as of
2026-05-28**. Repo `foreverlasting/ReSide` (default branch `main`, local
working branch `automation-layer`) and the GPL-fork `foreverlasting/Sideloader`
(branch `reside-automation`) are both public. **v0.4.1 release is published**
at <https://github.com/foreverlasting/ReSide/releases/tag/v0.4.1> (sha256
`6312a2dfa81029b0f220235f7f984efc798e58e2bf54a6231875e1934f70bf57`). v0.4.1
supersedes a never-published v0.4.0 draft; it rolled in four hardening fixes
after v0.4.0's hardware test on KDE Plasma 6 / CachyOS / Wayland: Wayland
launch fix (#5), pre-public polish (#6), multi-size icon install + system tray
(#7), catch_unwind around tray init + absolute `Icon=` path for KDE (#8).
Subsequent releases will follow the same flow — `packaging/RELEASING.md`
remains the canonical checklist.

**Working norms (non-negotiable):**
- The user is **not a developer**. Explain plainly, use analogies, ground jargon.
  End each round with one concrete thing for them to check on hardware/screen.
- Commit only when asked. Don't bump pinned deps (incl. the fork's LDC 1.34).
- Diagnostics via `tracing`, never `println!`. Keep the four gates green
  (`cargo test -p reside-core`, `clippy -D warnings`, `fmt --check`, `pnpm build`).
- New UI: never put `data-theme` and a `dark:` utility on the same node.
- Run the GUI with `pnpm tauri:dev` (sets the Wayland flags this machine needs).
- Device/Apple behavior validates only on the user's hardware (iPhone iOS 26.5,
  iPad Pro M1). Reset to new-user state: `scripts/reside-reset-newuser.sh`.
- Never bundle/commit Apple's ADI libraries; never copy the anisette device file
  between machines.

First message to the user: confirm you've read the architecture + roadmap, then
propose a concrete plan for the cert-management UI before writing code.

---

> Note: this repo may go public. Before the flip, review `docs/` for anything you
> don't want public — none of it contains secrets, but the handoff/roadmap framing
> is internal. Agent working memory (`~/.claude/.../memory/`) is **local only** and
> will not survive a disk wipe; its durable facts have been mirrored into
> `docs/ARCHITECTURE.md`.
