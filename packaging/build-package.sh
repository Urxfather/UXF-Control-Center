#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
FORMAT="${1:-}"
LIBDIR="${LIBDIR:-/usr/lib}"
LIB_SUBDIR="${LIBDIR#/usr/}"
BUILD_ROOT="$ROOT_DIR/target/package-build"
STAGE="$ROOT_DIR/target/package-stage"

# Ensure nfpm is accessible if in the known local build directory
if ! command -v nfpm >/dev/null 2>&1; then
    if [ -x "/home/uxf01/acer-control-center-v2.0.1-build/bin/nfpm" ]; then
        export PATH="/home/uxf01/acer-control-center-v2.0.1-build/bin:$PATH"
    fi
fi

case "$FORMAT" in
    deb|rpm|archlinux) ;;
    *) echo "Usage: $0 {deb|rpm|archlinux}" >&2; exit 2 ;;
esac
case "$LIBDIR" in
    /usr/lib|/usr/lib64) ;;
    *) echo "LIBDIR must be /usr/lib or /usr/lib64" >&2; exit 2 ;;
esac

command -v meson >/dev/null
command -v cargo >/dev/null
command -v nfpm >/dev/null
command -v npm >/dev/null

echo "=== Validating Version Parity Across Components ==="
bash "$ROOT_DIR/scripts/validate-versions.sh"

rm -rf "$STAGE" "$BUILD_ROOT/meson"
mkdir -p "$STAGE" "$BUILD_ROOT" "$ROOT_DIR/dist"

echo "=== Building libacercontrol (Meson) ==="
meson setup "$BUILD_ROOT/meson" "$ROOT_DIR/lib" --prefix=/usr --libdir="$LIB_SUBDIR" --buildtype=release
meson compile -C "$BUILD_ROOT/meson"
DESTDIR="$STAGE" meson install -C "$BUILD_ROOT/meson"

echo "=== Building acer-control-daemon (Cargo release) ==="
ACERCONTROL_LIB_DIR="$STAGE$LIBDIR" \
ACERCONTROL_RUNTIME_LIB_DIR="$LIBDIR" \
CARGO_TARGET_DIR="$BUILD_ROOT/cargo" \
cargo build --release --manifest-path "$ROOT_DIR/daemon/Cargo.toml"

echo "=== Building acer-control-center eframe fallback GUI (Cargo release) ==="
ACERCONTROL_LIB_DIR="$STAGE$LIBDIR" \
ACERCONTROL_RUNTIME_LIB_DIR="$LIBDIR" \
CARGO_TARGET_DIR="$BUILD_ROOT/cargo" \
cargo build --release --manifest-path "$ROOT_DIR/gui/Cargo.toml"

echo "=== Building UXF-Control-Center Tauri GUI (Vite + Cargo release) ==="
cd "$ROOT_DIR/gui-tauri"
npm run build
UXF_PKG_FULL_VERSION="${VERSION:-2.0.2}-3" cargo build --release --manifest-path "$ROOT_DIR/gui-tauri/src-tauri/Cargo.toml"
cd "$ROOT_DIR"

echo "=== Staging binaries and files ==="
install -Dm755 "$ROOT_DIR/gui-tauri/src-tauri/target/release/uxf-control-center-tauri" "$STAGE/usr/bin/uxf-control-center"
ln -sf uxf-control-center "$STAGE/usr/bin/uxf-control-center-tauri"
install -Dm755 "$BUILD_ROOT/cargo/release/acer-control-daemon" "$STAGE/usr/bin/acer-control-daemon"
install -Dm755 "$BUILD_ROOT/cargo/release/acer-control-center" "$STAGE/usr/bin/acer-control-center"

install -d "$STAGE/usr/share/acer-control-center"
cp -a "$ROOT_DIR/gui/resources/." "$STAGE/usr/share/acer-control-center/"

install -d "$STAGE/usr/src/acer-keyboard-0.1.1"
cp -a "$ROOT_DIR/driver/Makefile" "$ROOT_DIR/driver/dkms.conf" "$ROOT_DIR/driver/src" "$ROOT_DIR/driver/include" "$STAGE/usr/src/acer-keyboard-0.1.1/"

VERSION="${VERSION:-2.0.2}"
echo "=== Packaging $FORMAT with nfpm (version $VERSION) ==="
VERSION="$VERSION" \
LIB_SUBDIR="$LIB_SUBDIR" \
nfpm package --config "$ROOT_DIR/packaging/nfpm.yaml" --packager "$FORMAT" --target "$ROOT_DIR/dist"

echo "Built $FORMAT package(s) in $ROOT_DIR/dist"
