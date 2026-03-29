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
- Read-only `devfs` namespace bootstrap is online on `x86_64`
- Minimal VFS mount and read facade is online on `x86_64`
- VFS normalized path resolution and node lookup facade are online on `x86_64`
- `cpio` `newc` initrd discovery and `/initrd` read path are online on `x86_64`
- `/initrd/init` executable candidate discovery and format probe are online on `x86_64`
- `/initrd/init` ELF64 header and program-header inspection skeleton is online on `x86_64`
- `/initrd/init` ELF `PT_LOAD` vm-map planning with RWX and BSS accounting is online on `x86_64`
- Early Unix-like shebang interpreter fallback from `/bin/*` to `/initrd/bin/*` is online on `x86_64`
- Partial Linux + Ghost + HXNU-native syscall compatibility dispatcher bootstrap is online on `x86_64`
- `x86_64` `int 0x80` syscall gate, register-frame dispatch, and entry self-test are online
- Bootstrap `uaccess` copyin/copyout validation facade is online on `x86_64`
- Bootstrap `openat/ioctl/access/newfstatat/faccessat/faccessat2/readlinkat/dup/dup2/dup3/fcntl/getcwd/chdir/fchdir/read/fstat/getdents64/lseek/close` (`Linux`) and `open/ioctl/access/stat/readlink/dup/dup2/dup3/fcntl/getcwd/chdir/fchdir/read/fstat/getdents/seek/close` (`Ghost`, `HXNU`) VFS-backed syscall paths are online
- `exit_group` syscall path is connected to scheduler thread-exit request handling
- Scheduler-backed `getpid/getppid/gettid` identity path is online for bootstrap syscall personalities
- Process-scoped `umask`, root-identity `getuid/getgid/geteuid/getegid`, and `set_tid_address` paths are online for Linux/Ghost/HXNU bootstrap personalities
- Bootstrap anonymous `mmap/mprotect/munmap` and process-scoped `brk` syscall facades are online for Linux/Ghost/HXNU personalities
- Bootstrap `nanosleep/gettimeofday/getrandom` syscall facades are online for Linux/Ghost/HXNU personalities
- Bootstrap `rt_sigaction/rt_sigprocmask` syscall facades are online for Linux/Ghost/HXNU personalities
- Open-file table ownership is now process-scoped, and `exit_group` purges owned descriptors
- `exit_group` now tears down the current thread-group and advances to the next runnable scheduler entry
- Ghost and HXNU-native parent-process identity calls are online (`getppid` / `process_parent`)
- Multiple virtual TTY screen foundation is online on `x86_64`
- Scheduler thread table and runqueue skeleton are online on `x86_64`
- Bootstrap to idle-thread context switching is online on `x86_64`
- Styled framebuffer console output is online on `x86_64`
- Breakpoint, page fault, and general protection fault self-tests are working
- Power-reset self-test reaches the FADT reset-register path on `x86_64`
- Broader scheduler work remains next

Cross-repo status (as of 2026-03-29):
- External compiler repository `hxnu-rustc-compiler-x86_64` is online and versioned separately
- Rust-first SDK `v0.1.0` is tagged and includes `hxnu-rustc`, `hxnu-cargo`, `hxnu-sdk`, and `x86_64-unknown-hxnu` target spec
- SDK bundle flow (`build`, `pack`, `install`) and ELF verification flow are automated in the compiler repository
- Kernel integration model is consumer-style (`PATH` + `hxnu-cargo`), with no monorepo coupling

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
- Multiple virtual TTY screens or virtual consoles
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
- Active TTY switching and console session routing
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
- Rust cross compiler support with `x86_64` and `aarch64` as first-class targets (`x86_64` bootstrap release is online in external compiler repo)
- C and C++ cross compiler support with `x86_64` and `aarch64` as first-class targets
- Additional architectures after the main two are stable
- PowerISA 64-bit bring-up
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
- Multiple virtual TTY screens should sit between the early console path and full PTY/session semantics
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
- Compiler development continues in a dedicated repository: `https://github.com/neonix-bmx/hxnu-rustc-compiler-x86_64`
- Kernel repository tracks integration contract and acceptance checks, not compiler internals
