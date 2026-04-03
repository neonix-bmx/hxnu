#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="${ROOT}/build"
VENDOR_VERSION="${LIMINE_VERSION:-9.2.0}"
LIMINE_DIR="${ROOT}/vendor/limine-${VENDOR_VERSION}"
BASE_ISO_ROOT="${BUILD_DIR}/iso"
SMOKE_WORK_DIR="${BUILD_DIR}/fat-smoke"
SMOKE_INITRD_MIN="${SMOKE_WORK_DIR}/initrd-min.cpio"
SMOKE_INITRD_PATCHED="${SMOKE_WORK_DIR}/initrd-gpt-fat.cpio"
SMOKE_ISO_ROOT="${SMOKE_WORK_DIR}/iso-root"
SMOKE_ISO="${BUILD_DIR}/hxnu-fat-smoke.iso"
SMOKE_LOG="${BUILD_DIR}/qemu-fat-smoke.log"
SMOKE_TIMEOUT="${HXNU_FAT_SMOKE_TIMEOUT:-10}"

require_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "HXNU: missing required tool: $1" >&2
        exit 1
    fi
}

assert_log() {
    local pattern="$1"
    if ! grep -En "$pattern" "${SMOKE_LOG}" >/dev/null 2>&1; then
        echo "HXNU: FAT smoke assertion failed: ${pattern}" >&2
        echo "HXNU: showing last log lines (${SMOKE_LOG})" >&2
        tail -n 80 "${SMOKE_LOG}" >&2 || true
        exit 1
    fi
}

if ! [[ "${SMOKE_TIMEOUT}" =~ ^[0-9]+$ ]] || [ "${SMOKE_TIMEOUT}" -eq 0 ]; then
    echo "HXNU: HXNU_FAT_SMOKE_TIMEOUT must be a positive integer (seconds)" >&2
    exit 1
fi

require_tool cpio
require_tool python3
require_tool xorriso
require_tool qemu-system-x86_64

echo "HXNU: building baseline ISO"
"${ROOT}/scripts/build-iso.sh"

rm -rf "${SMOKE_WORK_DIR}"
mkdir -p "${SMOKE_WORK_DIR}" "${SMOKE_ISO_ROOT}"

TMP_INITRD_DIR="$(mktemp -d "${SMOKE_WORK_DIR}/initrd-src.XXXXXX")"
trap 'rm -rf "${TMP_INITRD_DIR}"' EXIT

cat > "${TMP_INITRD_DIR}/init" <<'EOF'
#!/init
EOF
chmod +x "${TMP_INITRD_DIR}/init"

(
    cd "${TMP_INITRD_DIR}"
    find . -print | LC_ALL=C sort | cpio -o -H newc --quiet > "${SMOKE_INITRD_MIN}"
)

python3 - "${SMOKE_INITRD_MIN}" "${SMOKE_INITRD_PATCHED}" <<'PY'
import sys
from pathlib import Path

src = Path(sys.argv[1])
out = Path(sys.argv[2])
data = bytearray(src.read_bytes())

SECTOR = 512
part_start = 64
part_sectors = 5000
required_sectors = part_start + part_sectors
required_bytes = required_sectors * SECTOR
if len(data) < required_bytes:
    data.extend(b"\x00" * (required_bytes - len(data)))

total_sectors = len(data) // SECTOR

# GPT header at LBA1.
header = bytearray(SECTOR)
header[0:8] = b"EFI PART"
header[8:12] = (0x00010000).to_bytes(4, "little")
header[12:16] = (92).to_bytes(4, "little")
header[24:32] = (1).to_bytes(8, "little")
header[32:40] = (total_sectors - 1).to_bytes(8, "little")
header[40:48] = (34).to_bytes(8, "little")
header[48:56] = (part_start + part_sectors - 1).to_bytes(8, "little")
header[56:72] = bytes.fromhex("6a9c5d8fb4a34728a9d8d2b1a0c3f102")
header[72:80] = (2).to_bytes(8, "little")
header[80:84] = (128).to_bytes(4, "little")
header[84:88] = (128).to_bytes(4, "little")
data[SECTOR : 2 * SECTOR] = header

# GPT entry array at LBA2; first entry only.
entry_base = 2 * SECTOR
entry = bytearray(128)
entry[0:16] = bytes.fromhex("a2a0d0ebe5b9334487c068b6b72699c7")
entry[16:32] = bytes.fromhex("de4f5bca7a0b4f2995e1666d0a4e2f11")
entry[32:40] = (part_start).to_bytes(8, "little")
entry[40:48] = (part_start + part_sectors - 1).to_bytes(8, "little")
name = "HXNUFAT".encode("utf-16le")
entry[56 : 56 + len(name)] = name
data[entry_base : entry_base + 128] = entry

# FAT16 BPB at partition start.
partition_base = part_start * SECTOR
boot = bytearray(SECTOR)
boot[0:3] = b"\xEB\x3C\x90"
boot[3:11] = b"HXNUFAT "
boot[11:13] = (512).to_bytes(2, "little")
boot[13] = 1
boot[14:16] = (1).to_bytes(2, "little")
boot[16] = 2
boot[17:19] = (512).to_bytes(2, "little")
boot[19:21] = (part_sectors).to_bytes(2, "little")
boot[21] = 0xF8
boot[22:24] = (16).to_bytes(2, "little")
boot[24:26] = (63).to_bytes(2, "little")
boot[26:28] = (255).to_bytes(2, "little")
boot[28:32] = (part_start).to_bytes(4, "little")
boot[32:36] = (0).to_bytes(4, "little")
boot[36] = 0x80
boot[38] = 0x29
boot[39:43] = (0x48584E55).to_bytes(4, "little")
boot[43:54] = b"HXNUFAT    "
boot[54:62] = b"FAT16   "
boot[510] = 0x55
boot[511] = 0xAA
data[partition_base : partition_base + SECTOR] = boot

reserved_sectors = 1
fat_count = 2
sectors_per_fat = 16
root_entries = 512
root_dir_sectors = ((root_entries * 32) + (SECTOR - 1)) // SECTOR
fat1_lba = part_start + reserved_sectors
fat2_lba = fat1_lba + sectors_per_fat
root_lba = part_start + reserved_sectors + (fat_count * sectors_per_fat)
first_data_lba = root_lba + root_dir_sectors

for fat_lba in (fat1_lba, fat2_lba):
    offset = fat_lba * SECTOR
    data[offset + 0 : offset + 2] = (0xFFF8).to_bytes(2, "little")
    data[offset + 2 : offset + 4] = (0xFFFF).to_bytes(2, "little")
    data[offset + 4 : offset + 6] = (0xFFFF).to_bytes(2, "little")
    data[offset + 6 : offset + 8] = (0xFFFF).to_bytes(2, "little")

root_offset = root_lba * SECTOR
hello_entry = bytearray(32)
hello_entry[0:11] = b"HELLO   TXT"
hello_entry[11] = 0x20
hello_entry[26:28] = (2).to_bytes(2, "little")
hello_entry[28:32] = (12).to_bytes(4, "little")
data[root_offset : root_offset + 32] = hello_entry

bin_entry = bytearray(32)
bin_entry[0:11] = b"BIN        "
bin_entry[11] = 0x10
bin_entry[26:28] = (3).to_bytes(2, "little")
data[root_offset + 32 : root_offset + 64] = bin_entry

data[root_offset + 64] = 0x00

payload = b"Hello HXNU!\n"
cluster2_offset = first_data_lba * SECTOR
data[cluster2_offset : cluster2_offset + len(payload)] = payload

out.write_bytes(data)
PY

cp -R "${BASE_ISO_ROOT}/." "${SMOKE_ISO_ROOT}/"
cp "${SMOKE_INITRD_PATCHED}" "${SMOKE_ISO_ROOT}/boot/initrd.cpio"

xorriso -as mkisofs -R -r -J -V HXNU \
    -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table -hfsplus \
    -apm-block-size 2048 \
    --efi-boot boot/limine/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image \
    --protective-msdos-label \
    "${SMOKE_ISO_ROOT}" -o "${SMOKE_ISO}"

if [ -x "${LIMINE_DIR}/limine" ]; then
    "${LIMINE_DIR}/limine" bios-install "${SMOKE_ISO}"
fi

QEMU_PREFIX="$(brew --prefix qemu 2>/dev/null || true)"
if [ -n "${QEMU_PREFIX}" ]; then
    QEMU_SHARE_DIR="${QEMU_PREFIX}/share/qemu"
else
    QEMU_SHARE_DIR="/opt/homebrew/share/qemu"
fi
UEFI_CODE="${QEMU_SHARE_DIR}/edk2-x86_64-code.fd"
UEFI_VARS_TEMPLATE="${QEMU_SHARE_DIR}/edk2-i386-vars.fd"
UEFI_VARS="${BUILD_DIR}/edk2-x86_64-vars-fat-smoke.fd"

rm -f "${SMOKE_LOG}"
if [ -f "${UEFI_CODE}" ] && [ -f "${UEFI_VARS_TEMPLATE}" ]; then
    cp "${UEFI_VARS_TEMPLATE}" "${UEFI_VARS}"
    qemu-system-x86_64 \
        -M q35,accel=tcg \
        -m 512M \
        -serial stdio \
        -display none \
        -drive if=pflash,format=raw,readonly=on,file="${UEFI_CODE}" \
        -drive if=pflash,format=raw,file="${UEFI_VARS}" \
        -cdrom "${SMOKE_ISO}" \
        -no-reboot \
        -no-shutdown > "${SMOKE_LOG}" 2>&1 &
else
    qemu-system-x86_64 \
        -M q35,accel=tcg \
        -m 512M \
        -serial stdio \
        -display none \
        -cdrom "${SMOKE_ISO}" \
        -no-reboot \
        -no-shutdown > "${SMOKE_LOG}" 2>&1 &
fi

QEMU_PID=$!
sleep "${SMOKE_TIMEOUT}"
kill -INT "${QEMU_PID}" 2>/dev/null || true
wait "${QEMU_PID}" 2>/dev/null || true

assert_log "HXNU: block online .*gpt-devices=1"
assert_log "HXNU: fat online table=gpt"
assert_log "HXNU: vfs online mounts=4"
assert_log "HXNU: vfs preview root=.*fat"
assert_log "HXNU: devfs preview sda=path /dev/sda"
assert_log "HXNU: devfs preview sda1=path /dev/sda1"
assert_log "HXNU: devfs preview nvme0n1=path /dev/nvme0n1"
assert_log "HXNU: devfs preview nvme0n1p1=path /dev/nvme0n1p1"
assert_log "HXNU: devfs preview nvm0n=path /dev/nvm0n"
assert_log "HXNU: devfs preview nvm0np1=path /dev/nvm0np1"
assert_log "HXNU: fat preview root="
assert_log "HXNU: init candidate path=/initrd/init"

echo "HXNU: FAT smoke acceptance passed"
grep -En "HXNU: (block online|fat online|vfs online|vfs preview root|devfs preview sda|devfs preview sda1|devfs preview nvme0n1|devfs preview nvme0n1p1|devfs preview nvm0n|devfs preview nvm0np1|fat preview root|init candidate)" "${SMOKE_LOG}"
echo "HXNU: iso=${SMOKE_ISO}"
echo "HXNU: log=${SMOKE_LOG}"
