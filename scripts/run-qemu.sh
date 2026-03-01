#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ISO_PATH="${ROOT}/build/hxnu.iso"
QEMU_PREFIX="$(brew --prefix qemu 2>/dev/null || true)"
if [ -n "${QEMU_PREFIX}" ]; then
    QEMU_SHARE_DIR="${QEMU_PREFIX}/share/qemu"
else
    QEMU_SHARE_DIR="/opt/homebrew/share/qemu"
fi
UEFI_CODE="${QEMU_SHARE_DIR}/edk2-x86_64-code.fd"
UEFI_VARS_TEMPLATE="${QEMU_SHARE_DIR}/edk2-i386-vars.fd"
UEFI_VARS="${ROOT}/build/edk2-x86_64-vars.fd"

if [ ! -f "${ISO_PATH}" ]; then
    "${ROOT}/scripts/build-iso.sh"
fi

if [ -f "${UEFI_CODE}" ] && [ -f "${UEFI_VARS_TEMPLATE}" ]; then
    cp "${UEFI_VARS_TEMPLATE}" "${UEFI_VARS}"
    exec qemu-system-x86_64 \
        -M q35,accel=tcg \
        -m 512M \
        -serial stdio \
        -display none \
        -drive if=pflash,format=raw,readonly=on,file="${UEFI_CODE}" \
        -drive if=pflash,format=raw,file="${UEFI_VARS}" \
        -cdrom "${ISO_PATH}" \
        -no-reboot \
        -no-shutdown
fi

exec qemu-system-x86_64 \
    -M q35,accel=tcg \
    -m 512M \
    -serial stdio \
    -display none \
    -cdrom "${ISO_PATH}" \
    -no-reboot \
    -no-shutdown
