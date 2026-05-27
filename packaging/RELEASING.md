# Releasing ReSide

Checklist for cutting a public GitHub Release of the `automation-layer` branch
(GPL-3.0). Plan: **GitHub Releases first, AUR later** (AUR sources from the
release). Nothing here is automated — every outward-facing step is a deliberate,
manual action.

> `gh` is not installed on this machine. Each publish step below gives both a
> `gh` command (if you install it: `sudo pacman -S github-cli`, then
> `gh auth login`) and a web-UI fallback.

## 0. Pre-flight — gates green (from `Sideloading/`)

```sh
cargo test -p reside-core
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
( cd crates/tauri-app && pnpm build )
```

Helpers built and current:
- `../sideloader-fork/bin/sideloader` (pinned ldc 1.34 build — see `env-sideloader-build`)
- `../netmuxd/target/release/netmuxd` (`cargo build --release`)

## 1. Build the release tarball

```sh
packaging/build-tarball.sh
# → target/release-tarball/ReSide-<version>-linux-x86_64.tar.gz  (+ printed sha256)
```

Keep the printed **sha256** — it goes in the release notes so users can verify.

## 2. Publish the Sideloader fork  ⚠️ GPL source-availability — REQUIRED

The tarball ships our **patched** `sideloader` binary (two ReSide patches:
non-interactive login + TLS-verify-on). GPL-3.0 requires the *source* of that
binary be available. So the fork must have a public home **before** the release
goes out — the README's build-from-source link must resolve.

- Upstream `origin` is `Dadoum/Sideloader` — **never push the fork there.**
- Publish to a fork repo under your own account (e.g. `everlasting-marshall/Sideloader`),
  pushing the `reside-automation` branch.

```sh
cd ../sideloader-fork
git remote add reside https://github.com/foreverlasting/Sideloader.git   # NOT origin/Dadoum
git push reside reside-automation
```

Web UI: fork `Dadoum/Sideloader` on GitHub, then push the `reside-automation`
branch to your fork.

Then update the README's "sideloader — the fork ReSide drives" link to point at
that repo (currently a placeholder).

## 3. netmuxd — no action needed (link only)

The shipped `netmuxd` is **unmodified upstream** `jkcoxson/netmuxd` at commit
`1c7dfd1`. LGPL-2.1 source-availability is satisfied by linking upstream at that
commit; the README already links it. (If netmuxd ever gets a ReSide patch, it
needs its own published fork like the signer above.)

## 4. Publish the ReSide repo

Push the `automation-layer` branch as the project's public main line. The
`main` / `native-signing-path` branches are a frozen snapshot of abandoned work —
decide whether to publish them at all (recommend: publish only `automation-layer`,
or rename it to `main` on the public remote).

```sh
cd ../Sideloading
git remote add origin https://github.com/foreverlasting/ReSide.git
git push -u origin automation-layer        # or: git push origin automation-layer:main
```

Web UI: create an empty repo, then push.

## 5. Create the GitHub Release

```sh
gh release create v<version> \
  target/release-tarball/ReSide-<version>-linux-x86_64.tar.gz \
  --title "ReSide v<version>" \
  --notes-file packaging/release-notes-v<version>.md
```

Web UI: Releases → Draft a new release → tag `v<version>` → upload the `.tar.gz`
as an asset → paste the notes.

## 6. Post-publish verification

- [ ] README's fork build-from-source link resolves (step 2).
- [ ] Download the released tarball on a clean path; `sha256sum` matches the notes.
- [ ] Extract → `./install.sh` → app launches from the menu → uninstall clean.
  (Already validated locally 2026-05-26; re-check after any rebuild.)

---

## Release notes — draft template

```markdown
# ReSide v<version>

A Linux-first desktop app that signs, sideloads, and **auto-refreshes** iOS apps
on your own iPhone/iPad — the automation, Wi-Fi, and reliability layer over a
proven signer.

## Install
Download `ReSide-<version>-linux-x86_64.tar.gz` below, then:
    tar -xzf ReSide-<version>-linux-x86_64.tar.gz
    cd ReSide-<version>-linux-x86_64
    ./install.sh
Runtime deps (Arch/CachyOS): `sudo pacman -S usbmuxd libimobiledevice webkit2gtk-4.1 libnotify`

## First-run expectations
- First sign-in downloads a one-time ~150 MB Apple component from Apple's CDN (needs internet).
- First sign-in asks for a 2FA code (one-time device trust, not per-app).
- A keyring (GNOME Keyring/KWallet) is only needed to save credentials and enable background auto-refresh.

## Requires
iOS / iPadOS 17.4 or newer (RemoteXPC / RSD transport).

## Verify your download
    sha256sum ReSide-<version>-linux-x86_64.tar.gz
    # <PASTE SHA256 FROM build-tarball.sh>

## What's inside / source
- ReSide (GPL-3.0) — this repo
- sideloader (GPL-3.0) — patched fork: <FORK URL>
- netmuxd (LGPL-2.1) — unmodified, jkcoxson/netmuxd @ 1c7dfd1
```
