#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${LIMINE_VERSION:-9.2.0}"
VENDOR_DIR="${ROOT}/vendor/limine-${VERSION}"
FALLBACK_DIR="${ROOT}/../heartix/target/limine-${VERSION}"
ARCHIVE_URL="https://github.com/limine-bootloader/limine/archive/refs/tags/v${VERSION}-binary.tar.gz"
TMP_DIR="${ROOT}/build/limine-tmp"

has_artifacts() {
    [ -f "$1/limine-bios.sys" ] && [ -f "$1/limine-bios-cd.bin" ] && [ -f "$1/limine-uefi-cd.bin" ]
}

mkdir -p "${ROOT}/vendor" "${ROOT}/build"

if has_artifacts "${VENDOR_DIR}"; then
    exit 0
fi

rm -rf "${VENDOR_DIR}"
mkdir -p "${VENDOR_DIR}"

if has_artifacts "${FALLBACK_DIR}"; then
    cp "${FALLBACK_DIR}"/limine-bios.sys "${VENDOR_DIR}/"
    cp "${FALLBACK_DIR}"/limine-bios-cd.bin "${VENDOR_DIR}/"
    cp "${FALLBACK_DIR}"/limine-uefi-cd.bin "${VENDOR_DIR}/"
    if [ -f "${FALLBACK_DIR}/BOOTX64.EFI" ]; then
        cp "${FALLBACK_DIR}/BOOTX64.EFI" "${VENDOR_DIR}/"
    fi
    if [ -f "${FALLBACK_DIR}/BOOTIA32.EFI" ]; then
        cp "${FALLBACK_DIR}/BOOTIA32.EFI" "${VENDOR_DIR}/"
    fi
    if [ -x "${FALLBACK_DIR}/limine" ]; then
        cp "${FALLBACK_DIR}/limine" "${VENDOR_DIR}/"
    fi
    exit 0
fi

rm -rf "${TMP_DIR}"
mkdir -p "${TMP_DIR}"
curl -L "${ARCHIVE_URL}" -o "${TMP_DIR}/limine.tar.gz"
tar -xzf "${TMP_DIR}/limine.tar.gz" -C "${TMP_DIR}"
EXTRACTED_DIR="$(find "${TMP_DIR}" -mindepth 1 -maxdepth 1 -type d | head -n 1)"
cp "${EXTRACTED_DIR}/limine-bios.sys" "${VENDOR_DIR}/"
cp "${EXTRACTED_DIR}/limine-bios-cd.bin" "${VENDOR_DIR}/"
cp "${EXTRACTED_DIR}/limine-uefi-cd.bin" "${VENDOR_DIR}/"
if [ -f "${EXTRACTED_DIR}/BOOTX64.EFI" ]; then
    cp "${EXTRACTED_DIR}/BOOTX64.EFI" "${VENDOR_DIR}/"
fi
if [ -f "${EXTRACTED_DIR}/BOOTIA32.EFI" ]; then
    cp "${EXTRACTED_DIR}/BOOTIA32.EFI" "${VENDOR_DIR}/"
fi
