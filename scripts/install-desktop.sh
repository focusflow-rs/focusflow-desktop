set -eu

APP_NAME="focusflow-desktop"
ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
BIN_SRC="$ROOT_DIR/target/release/$APP_NAME"
BIN_DEST="$HOME/.local/bin/$APP_NAME"
DESKTOP_SRC="$ROOT_DIR/packaging/focusflow-desktop.desktop"
DESKTOP_DEST="$HOME/.local/share/applications/focusflow-desktop.desktop"
AUTOSTART_DEST="$HOME/.config/autostart/focusflow-desktop.desktop"

ENABLE_AUTOSTART="false"
BUILD_RELEASE="true"

print_help() {
  cat <<'EOF'
Usage: sh scripts/install-desktop.sh [options]

Options:
  --autostart      Also install autostart entry (~/.config/autostart)
  --no-autostart   Ensure autostart entry is removed
  --no-build       Skip cargo build --release step
  -h, --help       Show this help

Examples:
  cargo build --release
  sh scripts/install-desktop.sh
  sh scripts/install-desktop.sh --autostart
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --autostart)
      ENABLE_AUTOSTART="true"
      ;;
    --no-autostart)
      ENABLE_AUTOSTART="false"
      ;;
    --no-build)
      BUILD_RELEASE="false"
      ;;
    -h|--help)
      print_help
      exit 0
      ;;
    *)
      echo "Unknown argument: $1"
      print_help
      exit 1
      ;;
  esac
  shift
done

if [ "$BUILD_RELEASE" = "true" ]; then
  echo "Building release binary..."
  (cd "$ROOT_DIR" && cargo build --release)
fi

if [ ! -f "$BIN_SRC" ]; then
  echo "Release binary not found at $BIN_SRC"
  echo "Build first with: cargo build --release"
  exit 1
fi

mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/.config/autostart"
cp "$BIN_SRC" "$BIN_DEST"
chmod +x "$BIN_DEST"

sed "s|^Exec=.*|Exec=$BIN_DEST|" "$DESKTOP_SRC" > "$DESKTOP_DEST"

if [ "$ENABLE_AUTOSTART" = "true" ]; then
  cp "$DESKTOP_DEST" "$AUTOSTART_DEST"
  echo "Installed autostart entry: $AUTOSTART_DEST"
else
  if [ -f "$AUTOSTART_DEST" ]; then
    rm -f "$AUTOSTART_DEST"
    echo "Removed autostart entry: $AUTOSTART_DEST"
  fi
fi

echo "Installed binary: $BIN_DEST"
echo "Installed desktop entry: $DESKTOP_DEST"
echo "You can now launch FocusFlow Desktop from the app menu."
