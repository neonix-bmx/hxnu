# HXNU Roadmap

Release target:
- `2605` as the first version marker for May 2026

## Phase 0
- Separate Rust kernel repository
- x86_64 target definition
- Minimal ELF kernel entry
- Early serial logging

## Phase 1
- Limine handoff wrappers
- Physical memory map parsing
- Early logging and panic reporting
- Frame allocator bootstrap
- Kernel heap bootstrap

## Phase 2
- GDT/IDT
- Exception handlers
- APIC or timer bring-up
- Interrupt dispatch
- Basic scheduler skeleton
- Structured kernel diagnostics and panic reports

Current status:
- GDT/IDT activation is online on `x86_64`
- CPUID inventory is online on `x86_64`
- CPUID topology leaf inventory from `0x0B/0x1F` is online on `x86_64`
- UEFI framebuffer or GOP handoff is online on `x86_64`
- Output-only TTY console bootstrap is online on `x86_64`
- Local APIC timer one-shot bring-up is online on `x86_64`
- Local APIC periodic tick and scheduler bootstrap are online on `x86_64`
- Minimal ACPI discovery with `RSDP`, `XSDT`, `MADT`, and `FADT` parsing is online on `x86_64`
- MADT processor, IO APIC, and interrupt-override topology summaries are online on `x86_64`
- FADT power and reset-register summaries are online on `x86_64`
- SMP topology inventory and AP bring-up target discovery are online on `x86_64`
- Read-only `procfs` snapshot bootstrap is online on `x86_64`
- Scheduler thread table and runqueue skeleton are online on `x86_64`
- Bootstrap to idle-thread context switching is online on `x86_64`
- Styled framebuffer console output is online on `x86_64`
- Breakpoint, page fault, and general protection fault self-tests are working
- Power-reset self-test reaches the FADT reset-register path on `x86_64`
- Broader scheduler work remains next

## Phase 3
- Virtual memory manager
- Kernel virtual address-space management
- User virtual address-space management
- Page-fault resolution path
- Process and thread core
- Syscall entry path
- User-kernel memory copy and validation path
- IPC fast path
- ELF loader
- VFS core
- `devfs`
- `procfs`
- TTY core and console plumbing
- Early keyboard or console input path
- UEFI framebuffer or GOP handoff and framebuffer console
- Block device layer
- Partition discovery
- cpio-compatible initrd support
- FAT16/32 support
- Minimal ACPI discovery on `x86_64`
- MADT and FADT parsing
- Reboot and poweroff plumbing
- Userspace ABI planning

## Phase 4
- SMP bring-up on `x86_64`
- BSP to AP startup flow
- Per-CPU data areas
- IPI support
- TLB shootdown path
- POSIX personality
- Legacy Ghost compatibility layer
- Core virtualization or LVE hooks
- Linux or Unix-like `init` startup contract
- PTY and POSIX terminal semantics
- Driver object model
- Device enumeration and bus framework
- Driver loading infrastructure for external driver directories
- Driver discovery and load policy for filesystem-backed modules
- Driver trust and load policy
- ext4 driver
- exFAT driver

## Phase 5
- aarch64 bring-up
- PL011 early UART
- DTB parsing
- Exception vectors and GIC
- aarch64 SMP topology bring-up
- Heterogeneous CPU topology support
- big.LITTLE or hybrid-core scheduling awareness
- Basic Ethernet bring-up
- Early network driver model
- Loopback and packet path groundwork
- Minimal userspace networking boundary

## Phase 6
- Rust cross compiler support with `x86_64` and `aarch64` as first-class targets
- C and C++ cross compiler support with `x86_64` and `aarch64` as first-class targets
- Additional architectures after the main two are stable
- PPC 32-bit bring-up
- Audio stack entry point
- Additional driver families loaded from external driver directories
- AHCI, NVMe, or virtio-blk expansion
- Richer Ethernet and audio driver families
- Debug monitor, symbol lookup, and crash dump direction

## Architecture Direction

- HXNU is a hybrid kernel
- Native HXNU primitives come first
- POSIX and legacy Ghost support are compatibility personalities, not the native kernel model
- Boot-critical and virtualization-critical pieces stay in kernel
- Replaceable services and policy should move to user space
- FAT16/32 can live in kernel if that keeps early boot and recovery simpler
- ext4 and exFAT are expected to work well as separate drivers or service modules
- `devfs` and `procfs` should arrive early with the VFS core
- TTY and framebuffer console support should be available before broader userspace compatibility work
- UEFI framebuffer support should be treated as a boot-critical display path
- Minimal ACPI discovery and power-state plumbing belong in kernel
- Full power-policy logic should stay outside the kernel when practical
- SMP comes before broad userspace compatibility work
- Heterogeneous CPU scheduling belongs after base SMP and timer stability
- The syscall and user-kernel boundary should be treated as a first-class kernel milestone
- Storage needs a block layer before filesystem work can scale
- Driver loading from dedicated filesystem directories should be supported after the base VFS and init path are stable

## Toolchain Priorities

- Rust cross compilation: `x86_64`, then `aarch64`
- C and C++ cross compilation: `x86_64`, then `aarch64`
- Other architectures only after the main two toolchains are reliable
