#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR_VERSION="${LIMINE_VERSION:-9.2.0}"
LIMINE_DIR="${ROOT}/vendor/limine-${VENDOR_VERSION}"
ISO_ROOT="${ROOT}/build/iso"
ISO_PATH="${ROOT}/build/hxnu.iso"
KERNEL_PATH="${ROOT}/target/x86_64-unknown-none/release/hxnu-kernel"

"${ROOT}/scripts/prepare-limine.sh"
"${ROOT}/scripts/build-initrd.sh"
if [ -n "${HXNU_CARGO_ARGS:-}" ]; then
    cargo build --release -p hxnu-kernel ${HXNU_CARGO_ARGS}
else
    cargo build --release -p hxnu-kernel
fi

rm -rf "${ISO_ROOT}"
mkdir -p "${ISO_ROOT}/boot/limine"
mkdir -p "${ISO_ROOT}/EFI/BOOT"
cp "${KERNEL_PATH}" "${ISO_ROOT}/boot/kernel"
cp "${ROOT}/build/initrd.cpio" "${ISO_ROOT}/boot/initrd.cpio"
cp "${ROOT}/boot/limine.conf" "${ISO_ROOT}/limine.conf"
cp "${ROOT}/boot/limine.conf" "${ISO_ROOT}/boot/limine/limine.conf"
cp "${LIMINE_DIR}/limine-bios.sys" "${ISO_ROOT}/limine-bios.sys"
cp "${LIMINE_DIR}/limine-bios.sys" "${ISO_ROOT}/boot/limine/limine-bios.sys"
cp "${LIMINE_DIR}/limine-bios-cd.bin" "${ISO_ROOT}/boot/limine/limine-bios-cd.bin"
cp "${LIMINE_DIR}/limine-uefi-cd.bin" "${ISO_ROOT}/boot/limine/limine-uefi-cd.bin"
if [ -f "${LIMINE_DIR}/BOOTX64.EFI" ]; then
    cp "${LIMINE_DIR}/BOOTX64.EFI" "${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI"
fi
if [ -f "${LIMINE_DIR}/BOOTIA32.EFI" ]; then
    cp "${LIMINE_DIR}/BOOTIA32.EFI" "${ISO_ROOT}/EFI/BOOT/BOOTIA32.EFI"
fi

xorriso -as mkisofs -R -r -J -V HXNU \
    -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table -hfsplus \
    -apm-block-size 2048 \
    --efi-boot boot/limine/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image \
    --protective-msdos-label \
    "${ISO_ROOT}" -o "${ISO_PATH}"

if [ -x "${LIMINE_DIR}/limine" ]; then
    "${LIMINE_DIR}/limine" bios-install "${ISO_PATH}"
fi
