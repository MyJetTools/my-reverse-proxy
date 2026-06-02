#!/usr/bin/env bash
# Build the Dioxus SPA in release mode (artifacts land in cargo `target/`)
# and then copy them into the server's wwwroot. The server
# (`my-reverse-proxy`) serves that folder via StaticFilesMiddleware, so a
# successful run of this script is the only step needed to publish a UI
# change.
#
# Usage:  ./build.sh
# Override target dir:  WWWROOT=/path/to/wwwroot ./build.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WWWROOT="${WWWROOT:-$SCRIPT_DIR/../my-reverse-proxy/wwwroot}"
DX_OUT="$SCRIPT_DIR/target/dx/my-reverse-proxy-ui/release/web/public"

cd "$SCRIPT_DIR"

# Wipe the dx output dir before building. dx keeps previously-built, content
# hashed bundles around in this folder, and the copy below would otherwise
# carry every stale .js/.wasm/.css into wwwroot. Removing it first guarantees
# wwwroot ends up with exactly the current bundle.
rm -rf "$SCRIPT_DIR/target/dx/my-reverse-proxy-ui"

dx build --release --platform web

if [ ! -d "$DX_OUT" ]; then
    echo "ERROR: expected dx build output at $DX_OUT but it was not produced." >&2
    exit 1
fi

# Wipe previous bundle so files removed from the UI don't linger.
rm -rf "$WWWROOT"
mkdir -p "$WWWROOT"

cp -R "$DX_OUT"/. "$WWWROOT/"

# Country flags are referenced dynamically (/assets/flags/<ISO3>.svg) from the
# logs dialog, so dx's static-asset bundler doesn't pick them up. Copy them in
# verbatim so the server can serve them.
if [ -d "$SCRIPT_DIR/assets/flags" ]; then
    mkdir -p "$WWWROOT/assets/flags"
    cp -R "$SCRIPT_DIR/assets/flags/." "$WWWROOT/assets/flags/"
fi

echo
echo "UI built  → $DX_OUT"
echo "Copied to → $WWWROOT"
