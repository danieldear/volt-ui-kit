#!/bin/sh
# Full Catalog: opt-in demo capabilities, identified macOS bundle, no automatic consent.
set -eu
case "${1:-}" in
  --help|-h)
    echo 'Usage: ./scripts/run_catalog.sh [--check]'
    echo 'Build the full GPU Catalog. macOS: bundle, ad-hoc sign and open it.'
    echo '--check builds/verifies the bundle without launching or requesting permission.'
    exit 0 ;;
  ''|--check) ;;
  *) echo 'Unknown option; use --help' >&2; exit 2 ;;
esac
[ "$#" -le 1 ] || { echo 'Too many arguments' >&2; exit 2; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
cd "$root"
# Ask Cargo for the executable path: supports CARGO_TARGET_DIR and target config.
receipt=$(mktemp "${TMPDIR:-/tmp}/volt-catalog-build.XXXXXX")
bundle_dir=''
cleanup() {
  rm -f -- "$receipt"
  if [ -n "$bundle_dir" ]; then rm -rf -- "$bundle_dir"; fi
}
trap cleanup EXIT
# An interrupted GUI launcher may leave the app alive. Keep that bundle rather
# than deleting the identity/resources of an application still running.
trap 'bundle_dir=""; exit 130' INT
trap 'bundle_dir=""; exit 143' TERM
cargo build --release --locked --features native-gpu,native-desktop,native-notifications \
  --example catalog --message-format=json-render-diagnostics > "$receipt"
binary=$(python3 - "$receipt" <<'PY'
import json, sys
found = []
for line in open(sys.argv[1]):
    message = json.loads(line)
    if (message.get('reason') == 'compiler-artifact'
        and message.get('target', {}).get('name') == 'catalog'
        and 'example' in message.get('target', {}).get('kind', [])
        and message.get('executable')):
        found.append(message['executable'])
if not found:
    sys.exit('Cargo did not produce the Catalog executable')
print(found[-1])
PY
)
if [ "$(uname -s)" = Darwin ]; then
  bundle_dir=$(mktemp -d "${TMPDIR:-/tmp}/volt-catalog-app.XXXXXX")
  app="$bundle_dir/Volt Catalog.app"
  cargo run --locked --features dev-tools --bin cargo-volt -- bundle \
    --binary "$binary" --output "$app" --name 'Volt Catalog' \
    --identifier com.volt.ui.catalog --version 0.5.0 \
    --minimum-system-version 11.0 --sign ad-hoc
  /usr/bin/codesign --verify --strict "$app"
  /usr/bin/plutil -lint "$app/Contents/Info.plist"
  if [ "${1:-}" != --check ]; then
    # UserNotifications resolves the requesting process through Launch Services.
    # Register BEFORE the process starts; open alone did not establish a valid
    # notification client for the fresh development bundle on this Mac.
    /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app"
    echo 'Opening Catalog. Desktop services requests permission only when you click.'
    /usr/bin/open -W -n "$app"
  fi
elif [ "${1:-}" != --check ]; then
  echo 'Local notifications are currently macOS-only; other demos remain available.'
  "$binary"
fi
