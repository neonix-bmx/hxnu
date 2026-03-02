# HXNU Architecture

HXNU is planned as a hybrid kernel.

This does not mean "some old monolithic code plus some random services outside the kernel". The rule is stricter:

- The kernel keeps only latency-sensitive, protection-sensitive, and virtualization-sensitive mechanisms.
- Policies and replaceable services are pushed out of the kernel whenever the performance and security model still make sense.
- Compatibility layers do not define the native kernel model.

## Native Kernel Model

HXNU should first define its own native primitives:

- threads
- processes and address spaces
- virtual memory objects and mappings
- IPC endpoints and messages
- interrupts and timers
- SMP and per-CPU execution state
- device objects
- filesystem and namespace primitives
- virtualization hooks

POSIX and legacy Ghost support should be built on top of these primitives, not the other way around.

## In Kernel

These components are expected to stay inside the kernel:

- architecture bring-up
- exception and interrupt entry
- scheduler and context switching
- SMP bring-up and inter-processor coordination
- physical memory management
- virtual memory management
- syscall entry and user-kernel boundary checks
- core IPC fast path
- process and thread lifecycle
- ELF loading
- VFS core
- block device layer
- page cache and block cache
- basic device model
- core virtualization or LVE support
- minimal boot-critical drivers
- TTY core
- `devfs` and `procfs`
- early framebuffer console support

## Outside Kernel

These components should be moved out when practical:

- init and service management policy
- GUI and window management
- audio stack
- network management policy
- input policy
- desktop services
- package and update services
- legacy Ghost userland services
- non-critical drivers
- terminal policy above the TTY core
- audio policy and higher-level media services

## Filesystem Direction

The filesystem stack should stay split by responsibility:

- VFS core in kernel
- `devfs` in kernel as the basic device namespace
- `procfs` in kernel for process and kernel introspection
- block device plumbing in kernel below filesystem implementations
- FAT16/32 can remain in kernel if that keeps early boot, recovery, and removable-media support simpler
- ext4 can live as a separate driver
- exFAT can live as a separate driver
- initrd support should be compatible with `cpio`

This keeps the boot path practical without forcing every filesystem implementation into the kernel image.

Before the full VFS arrives, `procfs` can start as a read-only kernel snapshot interface that generates pseudo-files directly from live kernel state. That gives early introspection without forcing a premature inode or mount implementation.

`devfs` can start the same way: a read-only device namespace with well-known nodes such as `console`, `tty0`, `null`, `zero`, and `kmsg`, backed by kernel-owned metadata until the full device model and VFS mount path are ready.

## SMP And Topology Direction

Multiprocessor support should arrive in layers:

- basic BSP-only stability first
- then AP startup and per-CPU data
- then IPI and TLB shootdown support
- only after stable SMP should heterogeneous scheduling be introduced

Heterogeneous CPU support should cover both:

- ARM `big.LITTLE`
- x86 hybrid-core topologies

This is primarily a scheduler and topology problem, not just a CPU enumeration problem.

## Console And Display Direction

The early display and terminal path should also stay layered:

- serial remains the first debug path
- UEFI framebuffer or GOP should provide the first graphical framebuffer path on `x86_64`
- framebuffer console support should exist before a full window system
- TTY core belongs in kernel
- PTY and richer terminal semantics can be added with the POSIX personality layer

This keeps early boot and recovery usable without forcing the full userspace terminal stack into the kernel.

## Power Management Direction

Power management should be introduced in layers:

- early `x86_64` support should focus on ACPI table discovery
- `MADT` is needed for interrupt topology and SMP work
- `FADT` is the first useful power-management table for reboot and shutdown
- reboot and poweroff plumbing can live in kernel
- power policy, thermal policy, and platform tuning should stay outside the kernel when practical

This keeps early platform support useful without dragging the project into a full AML and firmware-policy implementation too early.

## Syscall And ABI Direction

The syscall boundary should be treated as a core kernel interface, not as a late compatibility detail.

That means:

- explicit syscall entry and dispatch
- user-pointer validation
- copyin and copyout helpers
- a clear object or handle model
- separation between native HXNU ABI and compatibility personalities

This reduces the chance that the kernel ABI accidentally collapses into a POSIX or legacy Ghost internal model.

## Driver Loading Direction

HXNU should support both built-in drivers and filesystem-backed external drivers.

The intended progression is:

- boot-critical drivers built into the kernel image
- a dedicated driver directory discovered after VFS and `init` are stable
- explicit kernel support for loading approved drivers from that directory
- a trust and validation policy for loadable drivers
- later expansion toward Ethernet, audio, and other non-boot-critical driver families

This allows early bring-up to stay simple while preserving a path toward a cleaner driver deployment model.

## Compatibility Layers

HXNU is expected to provide two major compatibility personalities:

- POSIX personality
- legacy Ghost personality

These personalities should mostly be ABI and service layers:

- syscall surface
- object and handle translation
- process startup conventions
- filesystem layout expectations
- signal, IPC, and terminal semantics

The native HXNU kernel ABI should remain smaller and cleaner than either compatibility layer.

## Init And Process Startup

The `init` loading and startup model should follow Linux or Unix-like conventions.

That means:

- a single well-defined first user process
- Unix-like argv and environment passing
- Unix-like process startup expectations for libc and runtime code
- a filesystem-facing executable path for `init`
- a `cpio`-compatible initrd path for early userspace handoff

This requirement belongs to the userspace startup contract and POSIX-oriented personality layer. It should not force the native HXNU kernel object model to become Linux-internal in design.

## Hybrid Design Rules

To keep the design under control:

- Do not let POSIX semantics leak into the scheduler or VM core unless strictly required.
- Do not freeze the native kernel object model around Ghost-era assumptions.
- Keep fast paths in kernel, but keep service policy outside.
- Design virtualization and compatibility boundaries early, not after the syscall table grows.
- Prefer explicit kernel objects and message contracts over ad-hoc global subsystems.

## Initial Platform Order

- x86_64 first
- aarch64 second
- PPC 32-bit later

The x86_64 path should validate:

- boot handoff
- serial logging
- memory map parsing
- frame allocation
- page tables
- GDT and IDT
- exceptions
- timer
- scheduler skeleton
- syscall boundary basics
- block device basics
- minimal ACPI discovery
- SMP bootstrap
- TTY core
- UEFI framebuffer handoff
- `devfs` and `procfs`

Only after that should aarch64 receive:

- DTB handoff
- PL011 early UART
- exception vectors
- GIC
- MMU bring-up

## Compatibility Order

- native HXNU kernel core
- POSIX personality
- legacy Ghost compatibility

This order matters. If compatibility comes first, the new kernel will collapse back into a Ghost rewrite instead of becoming a real successor.

## Toolchain Direction

Cross-compilation support should be treated as a first-class part of the project:

- Rust cross compiler support for `x86_64` and `aarch64` first
- C and C++ cross compiler support for `x86_64` and `aarch64` first
- other architectures only after the two main targets are reliable
- PPC 32-bit belongs to the later expansion phase, not the initial kernel stabilization phase
