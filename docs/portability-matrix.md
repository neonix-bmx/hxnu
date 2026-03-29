# HXNU Portability Matrix (Bootstrap ABI)

Snapshot date:
- `2026-03-29`

Scope:
- Current `x86_64` bootstrap kernel state in this repository.
- Multi-personality syscall dispatcher is online (`Linux`, `Ghost`, `HXNU`), but this matrix classifies by practical Linux/Unix-style porting expectations.
- Focus is user-space program portability, not kernel feature completeness.

## Portability Levels

| Level | Program class | Current status | Typical blockers today | Next gate |
| --- | --- | --- | --- | --- |
| `L0` | `no_std` freestanding binaries (single-process style) | `Online` | Process model and rich POSIX are intentionally absent | Keep stable ABI + loader handoff wiring |
| `L1` | Tiny static CLI tools (`read/write`, simple args, no fork) | `Mostly online` | No persistent writable filesystem; limited process/session semantics | Writable VFS subset + argv/env handoff |
| `L2` | Small POSIX utilities (read-only FS traversal, metadata, polling) | `Partially online` | Missing `execve/fork/clone`, limited signal behavior, no PTY | Real `exec` path + process spawn primitives |
| `L3` | `musl` static apps with libc expectations (threads/signals/process control) | `Early` | libc contract incomplete (`crt`/sysroot/runtime semantics), missing scheduler/user ABI pieces | `musl` bootstrap sysroot + syscall contract hardening |
| `L4` | Shells, package tools, service daemons | `Not yet` | Missing full job control, PTY/session semantics, many FS mutation syscalls | PTY/session model + write-capable FS + process tree semantics |
| `L5` | Broad Linux userland compatibility | `Not yet` | Dynamic linking, large syscall surface (net, IPC, fs admin), behavioral parity | Toolchain + libc + kernel ABI conformance passes |

## Current Syscall Surface (Porting-Relevant)

Online bootstrap areas:
- FD I/O core: `read`, `write`, `close`, `dup/dup2/dup3`, `fcntl`, `readv/writev`, `pread64/pwrite64`
- Path and metadata (read-oriented): `open/openat`, `fstat/stat/newfstatat`, `readlink/readlinkat`, `access/faccessat/faccessat2`, `getdents/getdents64`, `getcwd/chdir/fchdir`, `lseek/seek`
- Memory and timing: `mmap/mprotect/munmap`, `brk`, `nanosleep`, `gettimeofday`, `clock_gettime`, `getrandom`
- Process identity/session subset: `getpid/getppid/gettid`, `setpgid/getpgid`, `setsid/getsid`, `wait4`, `exit/exit_group`
- Signals/robustness subset: `rt_sigaction`, `rt_sigprocmask`, `set_tid_address`, `set_robust_list/get_robust_list`, `rseq`, `futex (wait/wake)`
- Polling and pipes subset: `pipe/pipe2`, `poll/ppoll`
- Platform/ABI support: `uname`, `prctl`, `arch_prctl`

Still limiting general Unix/Linux ports:
- Process creation and program replacement are not complete (`fork/vfork/clone/execve` class is not online as a real userspace contract).
- Writable filesystem semantics are minimal for normal files (many apps expect create/modify/rename/unlink/mkdir/chmod/chown flows).
- IPC/network-heavy syscall families are not in bootstrap scope yet (sockets and related flows).
- PTY/job-control/session behavior is not at shell-grade parity yet.
- Dynamic loader/shared-library expectations are not available.

## Practical Porting Guidance (Now)

Best-first targets now:
- Small Rust static tools (`no_std` or minimal `std` assumptions).
- Read-only diagnostic utilities that do not require spawn/exec trees.
- Single-process init-like binaries with deterministic syscall footprint.

Defer until next milestones:
- Full shell stacks, package managers, language runtimes with complex process trees.
- Anything assuming writable root filesystem and dynamic linking.

## Suggested Near-Term Acceptance Gates

1. Real `exec` activation path:
- Consume current `/initrd/init` materialized `PT_LOAD` images and hand off entry safely.

2. Spawn contract:
- Introduce minimal `clone/fork+exec` path sufficient for parent/child orchestration tests.

3. Writable VFS baseline:
- Add constrained create/write/truncate/unlink/rename behavior for userland smoke tests.

4. libc bootstrap:
- Land `musl` bootstrap (`crt` objects + sysroot packaging + ABI validation set).

5. Toolchain expansion:
- Add `gcc-hxnu`/binutils track after Rust-first pipeline remains stable.
