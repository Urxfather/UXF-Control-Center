#!/usr/bin/env bash
set -euo pipefail

# scripts/validate-versions.sh
# Verifies version consistency across all UXF-Control-Center configuration files,
# packaging manifests, frontend state, and backend fallbacks.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "--> Checking version synchronization across UXF-Control-Center..."

EXPECTED_BASE_VERSION="2.0.2"
EXPECTED_RELEASE="3"
EXPECTED_FULL_VERSION="${EXPECTED_BASE_VERSION}-${EXPECTED_RELEASE}"

ERRORS=0

# 1. packaging/nfpm.yaml
NFPM_RELEASE=$(grep -E '^\s*release:' "$ROOT_DIR/packaging/nfpm.yaml" | awk '{print $2}' | tr -d '"'\')
if [ "$NFPM_RELEASE" != "$EXPECTED_RELEASE" ]; then
    echo "ERROR: packaging/nfpm.yaml release is '$NFPM_RELEASE', expected '$EXPECTED_RELEASE'" >&2
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] packaging/nfpm.yaml release: $NFPM_RELEASE"
fi

# 2. gui-tauri/src-tauri/Cargo.toml
CARGO_VER=$(grep -E '^\s*version\s*=' "$ROOT_DIR/gui-tauri/src-tauri/Cargo.toml" | head -n 1 | awk -F '"' '{print $2}')
if [ "$CARGO_VER" != "$EXPECTED_BASE_VERSION" ]; then
    echo "ERROR: gui-tauri/src-tauri/Cargo.toml version is '$CARGO_VER', expected '$EXPECTED_BASE_VERSION'" >&2
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] gui-tauri/src-tauri/Cargo.toml version: $CARGO_VER"
fi

# 3. gui-tauri/package.json
NPM_VER=$(grep -E '^\s*"version":' "$ROOT_DIR/gui-tauri/package.json" | head -n 1 | awk -F '"' '{print $4}')
if [ "$NPM_VER" != "$EXPECTED_BASE_VERSION" ]; then
    echo "ERROR: gui-tauri/package.json version is '$NPM_VER', expected '$EXPECTED_BASE_VERSION'" >&2
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] gui-tauri/package.json version: $NPM_VER"
fi

# 4. gui-tauri/src-tauri/tauri.conf.json
TAURI_VER=$(grep -E '^\s*"version":' "$ROOT_DIR/gui-tauri/src-tauri/tauri.conf.json" | head -n 1 | awk -F '"' '{print $4}')
if [ "$TAURI_VER" != "$EXPECTED_BASE_VERSION" ]; then
    echo "ERROR: gui-tauri/src-tauri/tauri.conf.json version is '$TAURI_VER', expected '$EXPECTED_BASE_VERSION'" >&2
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] gui-tauri/src-tauri/tauri.conf.json version: $TAURI_VER"
fi

# 5. packaging/build-package.sh
if ! grep -q "UXF_PKG_FULL_VERSION=.*${EXPECTED_RELEASE}" "$ROOT_DIR/packaging/build-package.sh"; then
    echo "ERROR: packaging/build-package.sh does not configure release ${EXPECTED_RELEASE}" >&2
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] packaging/build-package.sh UXF_PKG_FULL_VERSION configured with release $EXPECTED_RELEASE"
fi

# 6. gui-tauri/src/App.tsx
if ! grep -q "useState(\"${EXPECTED_FULL_VERSION}\")" "$ROOT_DIR/gui-tauri/src/App.tsx"; then
    echo "ERROR: gui-tauri/src/App.tsx initial appVersion does not match ${EXPECTED_FULL_VERSION}" >&2
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] gui-tauri/src/App.tsx initial appVersion: $EXPECTED_FULL_VERSION"
fi

# 7. gui-tauri/src-tauri/src/health.rs fallback
if ! grep -q "unwrap_or(\"${EXPECTED_FULL_VERSION}\")" "$ROOT_DIR/gui-tauri/src-tauri/src/health.rs"; then
    echo "ERROR: gui-tauri/src-tauri/src/health.rs default fallback does not match ${EXPECTED_FULL_VERSION}" >&2
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] gui-tauri/src-tauri/src/health.rs fallback: $EXPECTED_FULL_VERSION"
fi

if [ $ERRORS -gt 0 ]; then
    echo "FAILED: $ERRORS version mismatch(es) found! Aborting package build." >&2
    exit 1
fi

echo "--> All components synchronized successfully at ${EXPECTED_FULL_VERSION}."
exit 0
