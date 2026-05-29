# AGENTS.md

ReSide is a Linux-first desktop app (Tauri 2 + React/TS front end, Rust
`reside-core` back end) that signs, sideloads, and **auto-refreshes** iOS apps by
**driving a forked Dadoum Sideloader CLI** — it does **not** reimplement Apple's
signing stack. Solo maintainer, GPL-3.0, Arch/CachyOS-first.

This file is the entry point for any agent or tool working in the repo. It's a
**router, not a manual** — read the linked docs on demand to keep context small.

## Read first, in order

1. **`docs/ARCHITECTURE.md`** — how it works now. Critical: the **live-vs-parked
   map**. The live signing path is `crates/core/src/signer.rs` (drives the fork);
   the `signing/` module + `setup/adi_provision.rs` are an **abandoned native
   attempt** whose doc comments still say "Phase 2/future" — don't build on them.
2. **`docs/ROADMAP.md`** — prioritized work + current project state. Start at the
   `← start here` marker.
3. **Code + `git log` are ground truth.** `plan.md` / `Product-Brief.md` are
   historical (pre-pivot) and banner-marked as such — don't mine them for "how."

## Non-negotiable norms

- **The maintainer is not a developer.** Explain plainly, ground jargon, and end
  each round with one concrete thing for them to check on hardware/screen.
- **Commit only when asked.** Work on a short-lived feature branch off `main` and
  open a PR into `main` (the established flow — see PRs #4–#11); never commit
  straight to `main`. End commit messages with a `Co-Authored-By` trailer.
- **Keep the four gates green** (repo root): `cargo test -p reside-core`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --
  --check`, `(cd crates/tauri-app && pnpm build)`.
- **Don't bump pinned deps** (incl. the fork's LDC 1.34 toolchain).
- Diagnostics via `tracing`, never `println!`.
- New UI: never put `data-theme` and a `dark:` utility on the same node.
- Run the GUI with `pnpm tauri:dev` (sets the Wayland flags this setup needs);
  use `pnpm tauri:dev:local` to also point the helper binaries at the installed
  release ones (`~/.local/lib/reside/`), since the dev tree ships none.
- **Device/Apple behavior validates only on the maintainer's hardware.** Reset to
  new-user state with `scripts/reside-reset-newuser.sh`.
- Never bundle/commit Apple's ADI libraries; never copy the anisette device file
  between machines. Never push the fork to upstream `Dadoum/Sideloader`.

## Build

Commands + prerequisites: **README → "Building from source"**. Gates and the
pnpm/esbuild gotcha: **`docs/ARCHITECTURE.md` → "Build / run / gates"**. Human
contributors: **`CONTRIBUTING.md`**.

## Map

| Need | Go to |
|------|-------|
| How it works / live-vs-parked / gotchas | `docs/ARCHITECTURE.md` |
| What to work on / current state | `docs/ROADMAP.md` |
| Build & contribute (humans) | `CONTRIBUTING.md` |
| Cut a release | `packaging/RELEASING.md` |
