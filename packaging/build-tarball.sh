#!/usr/bin/env bash
#
# build-tarball.sh — assemble a self-contained ReSide release tarball.
#
# Tauri's bundler has no "tarball" target (only deb/rpm/appimage), so we build
# the release binary with `tauri build --no-bundle` and stage it ourselves
# alongside the two helper binaries ReSide spawns at runtime:
#
#   reside       the app                          (GPL-3.0)
#   sideloader   the forked Dadoum signer         (GPL-3.0)   — prebuilt, see env-sideloader-build
#   netmuxd      the on-demand Wi-Fi mux bridge    (LGPL-2.1)  — `cargo build --release`
#
# All three land in the SAME directory: reside-core resolves a helper "beside the
# running executable" (crates/core/src/locate.rs), so no env vars or absolute
# paths are baked in — and that absolute "beside" path is also what lets the
# unattended refresh agent self-configure its systemd unit.
#
# The helper binaries live outside this repo; point at them with env overrides if
# your checkout isn't laid out as ../sideloader-fork and ../netmuxd next to this
# repo:
#
#   RESIDE_SIDELOADER_SRC=/path/to/sideloader RESIDE_NETMUXD_SRC=/path/to/netmuxd \
#     packaging/build-tarball.sh
#
set -euo pipefail

# --- locate the repo root (this script lives in <repo>/packaging) ----------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# --- inputs ----------------------------------------------------------------
VERSION="$(grep -m1 '"version"' crates/tauri-app/src-tauri/tauri.conf.json | sed -E 's/.*"version" *: *"([^"]+)".*/\1/')"
ARCH="$(uname -m)"
NAME="ReSide-${VERSION}-linux-${ARCH}"

SIDELOADER_SRC="${RESIDE_SIDELOADER_SRC:-$REPO_ROOT/../sideloader-fork/bin/sideloader}"
NETMUXD_SRC="${RESIDE_NETMUXD_SRC:-$REPO_ROOT/../netmuxd/target/release/netmuxd}"

RESIDE_BIN="$REPO_ROOT/target/release/reside"
STAGE_DIR="$REPO_ROOT/target/release-tarball/$NAME"
OUT_TARBALL="$REPO_ROOT/target/release-tarball/${NAME}.tar.gz"

say() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

# --- preflight: helpers must exist before we spend minutes compiling --------
[ -f "$SIDELOADER_SRC" ] || die "sideloader not found at: $SIDELOADER_SRC
  Build it first (pinned ldc 1.34, see env-sideloader-build) or set RESIDE_SIDELOADER_SRC."
[ -f "$NETMUXD_SRC" ]    || die "netmuxd not found at: $NETMUXD_SRC
  Build it (cd netmuxd && cargo build --release) or set RESIDE_NETMUXD_SRC."

say "Building ReSide $VERSION for $ARCH"
say "  sideloader: $SIDELOADER_SRC"
say "  netmuxd:    $NETMUXD_SRC"

# --- build the app binary (frontend + Rust, no Tauri bundle) ----------------
# Two pnpm-11 escape hatches needed on this toolchain (esbuild ships a prebuilt
# native binary, so its postinstall is unnecessary — but pnpm 11 hard-errors on it):
#   --config.strictDepBuilds=false   `pnpm install` else exits 1 (ERR_PNPM_IGNORED_BUILDS).
#   --config.verifyDepsBeforeRun=false  `pnpm tauri build` else aborts on the same
#       pre-run check; pnpm-workspace.yaml pins it for plain `pnpm build` but the
#       setting isn't honored through `pnpm tauri build`'s spawned process.
say "Building frontend + release binary (tauri build --no-bundle)…"
( cd crates/tauri-app \
    && pnpm install --frozen-lockfile --config.strictDepBuilds=false \
    && pnpm --config.verifyDepsBeforeRun=false tauri build --no-bundle )
[ -f "$RESIDE_BIN" ] || die "expected binary not produced at $RESIDE_BIN"

# --- stage ------------------------------------------------------------------
say "Staging $NAME/"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"

install -m 0755 "$RESIDE_BIN"      "$STAGE_DIR/reside"
install -m 0755 "$SIDELOADER_SRC"  "$STAGE_DIR/sideloader"
install -m 0755 "$NETMUXD_SRC"     "$STAGE_DIR/netmuxd"

install -m 0755 "$SCRIPT_DIR/install.sh"   "$STAGE_DIR/install.sh"
install -m 0644 "$SCRIPT_DIR/reside.desktop" "$STAGE_DIR/reside.desktop"

# Stage the full icon set (SVG + multiple PNG sizes). install.sh fans these
# out into the hicolor theme so the menu, taskbar, alt-tab, and tray all find
# a crisp rendering. The list must match `ICON_INSTALLS` in install.sh.
mkdir -p "$STAGE_DIR/icons"
for f in icon.svg icon.png 32x32.png 128x128.png 128x128@2x.png; do
  install -m 0644 "crates/tauri-app/src-tauri/icons/$f" "$STAGE_DIR/icons/$f"
done

install -m 0644 README.md    "$STAGE_DIR/README.md"
install -m 0644 LICENSE      "$STAGE_DIR/LICENSE"
install -m 0644 LICENSES.md  "$STAGE_DIR/LICENSES.md"

# strip the binaries we built/ship to shrink the tarball (helpers may already be stripped)
strip --strip-unneeded "$STAGE_DIR/reside" "$STAGE_DIR/netmuxd" 2>/dev/null || true

# --- pack -------------------------------------------------------------------
say "Packing ${NAME}.tar.gz"
rm -f "$OUT_TARBALL"
tar -C "$(dirname "$STAGE_DIR")" -czf "$OUT_TARBALL" "$NAME"

# --- report -----------------------------------------------------------------
SIZE="$(du -h "$OUT_TARBALL" | cut -f1)"
SHA="$(sha256sum "$OUT_TARBALL" | cut -d' ' -f1)"
echo
say "Done: $OUT_TARBALL  ($SIZE)"
echo "  sha256: $SHA"
echo
echo "Contents:"
tar -tzf "$OUT_TARBALL" | sed 's/^/    /'
