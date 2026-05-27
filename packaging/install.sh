#!/usr/bin/env bash
#
# install.sh — install (or remove) ReSide for the current user.
#
# Installs into your home directory only — no root, no system files:
#
#   ~/.local/lib/reside/      reside, sideloader, netmuxd, reside.png  (kept together)
#   ~/.local/bin/reside       symlink onto the app  (so `reside` works from a terminal)
#   ~/.local/share/applications/dev.reside.app.desktop   (so it shows in your menu)
#   ~/.local/share/icons/hicolor/512x512/apps/reside.png
#
# The three binaries MUST stay in the same folder: ReSide finds the signer and
# Wi-Fi helper sitting beside itself. The launcher symlink resolves back to that
# folder, so menu launches and terminal launches behave identically.
#
# Usage:
#   ./install.sh              install for the current user
#   ./install.sh --uninstall  remove everything listed above
#
set -euo pipefail

SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PREFIX="${PREFIX:-$HOME/.local}"
LIBDIR="$PREFIX/lib/reside"
BINDIR="$PREFIX/bin"
APPDIR="$PREFIX/share/applications"
ICONDIR="$PREFIX/share/icons/hicolor/512x512/apps"

DESKTOP_FILE="$APPDIR/dev.reside.app.desktop"
LAUNCHER="$BINDIR/reside"

say() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }

uninstall() {
  say "Removing ReSide"
  rm -rf "$LIBDIR"
  rm -f  "$LAUNCHER" "$DESKTOP_FILE" "$ICONDIR/reside.png"
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPDIR" 2>/dev/null || true
  say "Removed. (Your signed apps, credentials, and app data under ~/.local/share/reside were left untouched.)"
  exit 0
}

[ "${1:-}" = "--uninstall" ] && uninstall

# --- install ----------------------------------------------------------------
for b in reside sideloader netmuxd; do
  [ -f "$SRC_DIR/$b" ] || { printf 'error: missing %s next to this script — is the tarball intact?\n' "$b" >&2; exit 1; }
done

say "Installing ReSide into $PREFIX"
mkdir -p "$LIBDIR" "$BINDIR" "$APPDIR" "$ICONDIR"

install -m 0755 "$SRC_DIR/reside"     "$LIBDIR/reside"
install -m 0755 "$SRC_DIR/sideloader" "$LIBDIR/sideloader"
install -m 0755 "$SRC_DIR/netmuxd"    "$LIBDIR/netmuxd"
install -m 0644 "$SRC_DIR/reside.png" "$LIBDIR/reside.png"
install -m 0644 "$SRC_DIR/reside.png" "$ICONDIR/reside.png"

ln -sf "$LIBDIR/reside" "$LAUNCHER"

# desktop entry: point Exec at the real installed binary so menu launches don't
# depend on ~/.local/bin being on the session PATH.
sed "s|__EXEC__|$LIBDIR/reside|" "$SRC_DIR/reside.desktop" > "$DESKTOP_FILE"
chmod 0644 "$DESKTOP_FILE"

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPDIR" 2>/dev/null || true

say "Installed."
echo "  Launch from your app menu (\"ReSide\"), or run: reside"

# nudge if ~/.local/bin isn't on PATH (terminal launch won't find `reside` otherwise)
case ":$PATH:" in
  *":$BINDIR:"*) : ;;
  *) echo
     echo "  Note: $BINDIR is not on your PATH, so the \`reside\` command won't be found"
     echo "  in a terminal. The app-menu entry still works. To enable the command, add:"
     echo "      set -gx PATH $BINDIR \$PATH        # fish"
     echo "      export PATH=\"$BINDIR:\$PATH\"       # bash/zsh"
     ;;
esac
echo
echo "  First sign-in downloads a one-time ~150 MB Apple component and asks for a 2FA code — see README."
echo "  To remove later:  ~/.local/lib/reside  →  re-run this script with --uninstall, or just delete that folder."
