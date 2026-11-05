#!/bin/sh
# Canonical build helper. The generated launcher provides Cargo metadata, not
# shell-interpolated app/package paths. No history, source trees or SDKs copied.
set -eu
META=${1:?Cargo metadata JSON required}
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
APP=$(python3 - "$META" <<'PY'
import json,sys
m=json.load(open(sys.argv[1]));root=m['resolve']['root']
p=next(p for p in m['packages'] if p['id']==root)
print(p['manifest_path'])
PY
)
LIB=$(python3 - "$META" <<'PY'
import json,sys
m=json.load(open(sys.argv[1]));p=next(p for p in m['packages'] if p['id']==m['resolve']['root'])
libs=[t['name'] for t in p['targets'] if 'staticlib' in t['crate_types']]
if len(libs)!=1:raise SystemExit('Volt iOS requires exactly one staticlib target')
print(libs[0])
PY
)
CACHE=$(python3 - "$META" <<'PY'
import json,sys
print(json.load(open(sys.argv[1]))['target_directory'])
PY
)
case "${PLATFORM_NAME:?Xcode SDK required}:${CURRENT_ARCH:-undefined_arch}" in
  iphonesimulator:arm64|iphonesimulator:undefined_arch) TARGET=aarch64-apple-ios-sim ;;
  iphonesimulator:x86_64) TARGET=x86_64-apple-ios ;;
  iphoneos:arm64|iphoneos:undefined_arch) TARGET=aarch64-apple-ios ;;
  *) echo "Unsupported SDK/architecture: $PLATFORM_NAME:${CURRENT_ARCH:-}" >&2; exit 1 ;;
esac
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"
cargo build --locked --release --manifest-path "$APP" --target "$TARGET"
mkdir -p "${BUILT_PRODUCTS_DIR:?Xcode products required}"
cp "$CACHE/$TARGET/release/lib$LIB.a" "$BUILT_PRODUCTS_DIR/libvolt_app.a"
