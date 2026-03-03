#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE_DIR="${ROOT}/initrd"
BUILD_DIR="${ROOT}/build"
ARCHIVE_PATH="${BUILD_DIR}/initrd.cpio"

mkdir -p "${BUILD_DIR}"
rm -f "${ARCHIVE_PATH}"

(
    cd "${SOURCE_DIR}"
    find . -print | LC_ALL=C sort | cpio -o -H newc --quiet > "${ARCHIVE_PATH}"
)
