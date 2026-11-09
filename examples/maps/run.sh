#!/bin/sh
# Build the maps example, bundle it as Maps.app and open it.
#
# macOS asks for location access on behalf of an app bundle with a usage
# description, opened through Launch Services, so the app is not run as a
# bare binary. Keys set in this shell (MAPBOX_ACCESS_TOKEN,
# GOOGLE_MAPS_API_KEY) are passed to the app; its output goes to a log file
# whose path is printed.
set -eu
cd "$(dirname "$0")/../.."
cargo build --release --features native-gpu --example maps

dir=$(mktemp -d "${TMPDIR:-/tmp}/volt-maps-app.XXXXXX")
app="$dir/Maps.app"
why="Maps shows where you are on the map and can start directions there."
cargo run --quiet --features dev-tools --bin cargo-volt -- bundle \
  --binary target/release/examples/maps --output "$app" --name Maps \
  --identifier dev.volt.examples.maps --version 0.1.0 \
  --minimum-system-version 11.0 \
  --usage "NSLocationUsageDescription=$why" --sign ad-hoc
# Newer macOS reads the when-in-use key for requestWhenInUseAuthorization;
# the bundler doesn't offer it yet, so add it and sign again.
/usr/bin/plutil -insert NSLocationWhenInUseUsageDescription -string "$why" "$app/Contents/Info.plist"
/usr/bin/codesign --force --sign - "$app" >/dev/null
/usr/bin/codesign --verify --strict "$app"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app"

set --
[ -n "${MAPBOX_ACCESS_TOKEN:-}" ] && set -- "$@" --env "MAPBOX_ACCESS_TOKEN=$MAPBOX_ACCESS_TOKEN"
[ -n "${GOOGLE_MAPS_API_KEY:-}" ] && set -- "$@" --env "GOOGLE_MAPS_API_KEY=$GOOGLE_MAPS_API_KEY"
[ -n "${MAPS_DEBUG:-}" ] && set -- "$@" --env MAPS_DEBUG=1
[ -n "${MAPS_DEMO:-}" ] && set -- "$@" --env MAPS_DEMO=1
[ -n "${VOLT_UI_TRACE:-}" ] && set -- "$@" --env VOLT_UI_TRACE=1
log="$dir/maps.log"
: >"$log"
echo "Opening $app"
echo "Log: $log"
/usr/bin/open -W -n --stdout "$log" --stderr "$log" "$@" "$app"
