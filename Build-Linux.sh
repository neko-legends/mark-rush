#!/usr/bin/env bash
# Builds the Linux release zip (release/linux/markrush-linux-x86_64.zip) holding
# the `markrush` binary plus license files. Neko Legends Control Center installs
# it on Linux: it accepts a zip whose name contains "linux" and launches the
# extensionless executable inside it.
#
# Usage: ./Build-Linux.sh [yyyyMMddHHmmss build stamp]
set -euo pipefail

BUILD_STAMP="${1:-20261107000000}"
if [[ ! "$BUILD_STAMP" =~ ^[0-9]{14}$ ]]; then
    echo "Build stamp must use yyyyMMddHHmmss format." >&2
    exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

export MARKRUSH_BUILD_STAMP="$BUILD_STAMP"
cargo build --release --locked

BINARY="$ROOT/target/release/markrush"
[[ -x "$BINARY" ]] || { echo "Expected executable was not produced: $BINARY" >&2; exit 1; }

ARCH="$(uname -m)"
OUT_DIR="$ROOT/release/linux"
STAGE="$OUT_DIR/stage"
PACKAGE="$OUT_DIR/markrush-linux-$ARCH.zip"
rm -rf "$OUT_DIR"
mkdir -p "$STAGE/font-licenses"

install -s -m 755 "$BINARY" "$STAGE/markrush"
install -m 644 README.md LICENSE "$STAGE/"
install -m 644 assets/fonts/README.md "$STAGE/font-licenses/README.md"
install -m 644 assets/fonts/OFL-*.txt "$STAGE/font-licenses/"

# zip stores unix mode bits, so `markrush` stays executable after extraction.
(cd "$STAGE" && zip -qr -X "$PACKAGE" .)
rm -rf "$STAGE"

HASH="$(sha256sum "$PACKAGE" | cut -d' ' -f1)"
echo "$HASH  $(basename "$PACKAGE")" > "$PACKAGE.sha256.txt"

echo "Linux zip: $PACKAGE"
echo "SHA256: $HASH"
