use alloc::alloc::{alloc_zeroed, dealloc};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::cmp::min;
use core::mem::{size_of, size_of_val};
use core::slice;
use core::str;

use crate::sched;
use crate::time;
use crate::tty;
use crate::uaccess::{self, UserCopyError};
use crate::vfs;
use crate::vfs::{VfsMountKind, VfsNodeKind};

pub const LINUX_ABI_NAME: &str = "linux-x86_64-bootstrap";
pub const GHOST_ABI_NAME: &str = "ghost-bootstrap";
pub const HXNU_ABI_NAME: &str = "hxnu-native-bootstrap";

pub const LINUX_SYS_READ: u64 = 0;
pub const LINUX_SYS_WRITE: u64 = 1;
pub const LINUX_SYS_CLOSE: u64 = 3;
pub const LINUX_SYS_FSTAT: u64 = 5;
pub const LINUX_SYS_MMAP: u64 = 9;
pub const LINUX_SYS_MPROTECT: u64 = 10;
pub const LINUX_SYS_MUNMAP: u64 = 11;
pub const LINUX_SYS_BRK: u64 = 12;
pub const LINUX_SYS_RT_SIGACTION: u64 = 13;
pub const LINUX_SYS_RT_SIGPROCMASK: u64 = 14;
pub const LINUX_SYS_PREAD64: u64 = 17;
pub const LINUX_SYS_PWRITE64: u64 = 18;
pub const LINUX_SYS_READV: u64 = 19;
pub const LINUX_SYS_WRITEV: u64 = 20;
pub const LINUX_SYS_NANOSLEEP: u64 = 35;
pub const LINUX_SYS_LSEEK: u64 = 8;
pub const LINUX_SYS_IOCTL: u64 = 16;
pub const LINUX_SYS_ACCESS: u64 = 21;
pub const LINUX_SYS_DUP: u64 = 32;
pub const LINUX_SYS_DUP2: u64 = 33;
pub const LINUX_SYS_SCHED_YIELD: u64 = 24;
pub const LINUX_SYS_GETPID: u64 = 39;
pub const LINUX_SYS_EXIT: u64 = 60;
pub const LINUX_SYS_WAIT4: u64 = 61;
pub const LINUX_SYS_UNAME: u64 = 63;
pub const LINUX_SYS_FCNTL: u64 = 72;
pub const LINUX_SYS_GETCWD: u64 = 79;
pub const LINUX_SYS_CHDIR: u64 = 80;
pub const LINUX_SYS_FCHDIR: u64 = 81;
pub const LINUX_SYS_UMASK: u64 = 95;
pub const LINUX_SYS_GETTIMEOFDAY: u64 = 96;
pub const LINUX_SYS_GETRLIMIT: u64 = 97;
pub const LINUX_SYS_GETUID: u64 = 102;
pub const LINUX_SYS_GETGID: u64 = 104;
pub const LINUX_SYS_GETEUID: u64 = 107;
pub const LINUX_SYS_GETEGID: u64 = 108;
pub const LINUX_SYS_SETPGID: u64 = 109;
pub const LINUX_SYS_GETPPID: u64 = 110;
pub const LINUX_SYS_SETSID: u64 = 112;
pub const LINUX_SYS_GETPGID: u64 = 121;
pub const LINUX_SYS_GETSID: u64 = 124;
pub const LINUX_SYS_PRCTL: u64 = 157;
pub const LINUX_SYS_SETRLIMIT: u64 = 160;
pub const LINUX_SYS_GETTID: u64 = 186;
pub const LINUX_SYS_GETDENTS64: u64 = 217;
pub const LINUX_SYS_SET_TID_ADDRESS: u64 = 218;
pub const LINUX_SYS_CLOCK_GETTIME: u64 = 228;
pub const LINUX_SYS_EXIT_GROUP: u64 = 231;
pub const LINUX_SYS_OPENAT: u64 = 257;
pub const LINUX_SYS_NEWFSTATAT: u64 = 262;
pub const LINUX_SYS_READLINKAT: u64 = 267;
pub const LINUX_SYS_FACCESSAT: u64 = 269;
pub const LINUX_SYS_SET_ROBUST_LIST: u64 = 273;
pub const LINUX_SYS_GET_ROBUST_LIST: u64 = 274;
pub const LINUX_SYS_DUP3: u64 = 292;
pub const LINUX_SYS_PRLIMIT64: u64 = 302;
pub const LINUX_SYS_GETRANDOM: u64 = 318;
pub const LINUX_SYS_RSEQ: u64 = 334;
pub const LINUX_SYS_FACCESSAT2: u64 = 439;

pub const GHOST_SYS_WRITE: u64 = 1;
pub const GHOST_SYS_YIELD: u64 = 2;
pub const GHOST_SYS_GETPID: u64 = 3;
pub const GHOST_SYS_GETTID: u64 = 4;
pub const GHOST_SYS_UPTIME_NSEC: u64 = 5;
pub const GHOST_SYS_UNAME: u64 = 6;
pub const GHOST_SYS_EXIT_GROUP: u64 = 7;
pub const GHOST_SYS_OPEN: u64 = 8;
pub const GHOST_SYS_READ: u64 = 9;
pub const GHOST_SYS_CLOSE: u64 = 10;
pub const GHOST_SYS_SEEK: u64 = 11;
pub const GHOST_SYS_GETPPID: u64 = 12;
pub const GHOST_SYS_FSTAT: u64 = 13;
pub const GHOST_SYS_STAT: u64 = 14;
pub const GHOST_SYS_GETDENTS: u64 = 15;
pub const GHOST_SYS_READLINK: u64 = 16;
pub const GHOST_SYS_ACCESS: u64 = 17;
pub const GHOST_SYS_IOCTL: u64 = 18;
pub const GHOST_SYS_DUP: u64 = 19;
pub const GHOST_SYS_DUP3: u64 = 20;
pub const GHOST_SYS_FCNTL: u64 = 21;
pub const GHOST_SYS_GETCWD: u64 = 22;
pub const GHOST_SYS_CHDIR: u64 = 23;
pub const GHOST_SYS_FCHDIR: u64 = 24;
pub const GHOST_SYS_DUP2: u64 = 25;
pub const GHOST_SYS_UMASK: u64 = 26;
pub const GHOST_SYS_GETUID: u64 = 27;
pub const GHOST_SYS_GETGID: u64 = 28;
pub const GHOST_SYS_GETEUID: u64 = 29;
pub const GHOST_SYS_GETEGID: u64 = 30;
pub const GHOST_SYS_SET_TID_ADDRESS: u64 = 31;
pub const GHOST_SYS_MMAP: u64 = 32;
pub const GHOST_SYS_MPROTECT: u64 = 33;
pub const GHOST_SYS_MUNMAP: u64 = 34;
pub const GHOST_SYS_BRK: u64 = 35;
pub const GHOST_SYS_NANOSLEEP: u64 = 36;
pub const GHOST_SYS_GETTIMEOFDAY: u64 = 37;
pub const GHOST_SYS_GETRANDOM: u64 = 38;
pub const GHOST_SYS_RT_SIGACTION: u64 = 39;
pub const GHOST_SYS_RT_SIGPROCMASK: u64 = 40;
pub const GHOST_SYS_PREAD64: u64 = 41;
pub const GHOST_SYS_PWRITE64: u64 = 42;
pub const GHOST_SYS_READV: u64 = 43;
pub const GHOST_SYS_WRITEV: u64 = 44;
pub const GHOST_SYS_WAIT4: u64 = 45;
pub const GHOST_SYS_SETPGID: u64 = 46;
pub const GHOST_SYS_GETPGID: u64 = 47;
pub const GHOST_SYS_SETSID: u64 = 48;
pub const GHOST_SYS_GETSID: u64 = 49;
pub const GHOST_SYS_GETRLIMIT: u64 = 50;
pub const GHOST_SYS_SETRLIMIT: u64 = 51;
pub const GHOST_SYS_PRLIMIT64: u64 = 52;
pub const GHOST_SYS_PRCTL: u64 = 53;
pub const GHOST_SYS_SET_ROBUST_LIST: u64 = 54;
pub const GHOST_SYS_GET_ROBUST_LIST: u64 = 55;
pub const GHOST_SYS_RSEQ: u64 = 56;

pub const HXNU_SYS_LOG_WRITE: u64 = 0x484e_0001;
pub const HXNU_SYS_THREAD_SELF: u64 = 0x484e_0002;
pub const HXNU_SYS_PROCESS_SELF: u64 = 0x484e_0003;
pub const HXNU_SYS_UPTIME_NSEC: u64 = 0x484e_0004;
pub const HXNU_SYS_SCHED_YIELD: u64 = 0x484e_0005;
pub const HXNU_SYS_ABI_VERSION: u64 = 0x484e_0006;
pub const HXNU_SYS_OPEN: u64 = 0x484e_0007;
pub const HXNU_SYS_READ: u64 = 0x484e_0008;
pub const HXNU_SYS_CLOSE: u64 = 0x484e_0009;
pub const HXNU_SYS_SEEK: u64 = 0x484e_000a;
pub const HXNU_SYS_PROCESS_PARENT: u64 = 0x484e_000b;
pub const HXNU_SYS_FSTAT: u64 = 0x484e_000c;
pub const HXNU_SYS_STAT: u64 = 0x484e_000d;
pub const HXNU_SYS_GETDENTS: u64 = 0x484e_000e;
pub const HXNU_SYS_READLINK: u64 = 0x484e_000f;
pub const HXNU_SYS_ACCESS: u64 = 0x484e_0010;
pub const HXNU_SYS_IOCTL: u64 = 0x484e_0011;
pub const HXNU_SYS_DUP: u64 = 0x484e_0012;
pub const HXNU_SYS_DUP3: u64 = 0x484e_0013;
pub const HXNU_SYS_FCNTL: u64 = 0x484e_0014;
pub const HXNU_SYS_GETCWD: u64 = 0x484e_0015;
pub const HXNU_SYS_CHDIR: u64 = 0x484e_0016;
pub const HXNU_SYS_FCHDIR: u64 = 0x484e_0017;
pub const HXNU_SYS_DUP2: u64 = 0x484e_0018;
pub const HXNU_SYS_UMASK: u64 = 0x484e_0019;
pub const HXNU_SYS_GETUID: u64 = 0x484e_001a;
pub const HXNU_SYS_GETGID: u64 = 0x484e_001b;
pub const HXNU_SYS_GETEUID: u64 = 0x484e_001c;
pub const HXNU_SYS_GETEGID: u64 = 0x484e_001d;
pub const HXNU_SYS_SET_TID_ADDRESS: u64 = 0x484e_001e;
pub const HXNU_SYS_MMAP: u64 = 0x484e_001f;
pub const HXNU_SYS_MPROTECT: u64 = 0x484e_0020;
pub const HXNU_SYS_MUNMAP: u64 = 0x484e_0021;
pub const HXNU_SYS_BRK: u64 = 0x484e_0022;
pub const HXNU_SYS_NANOSLEEP: u64 = 0x484e_0023;
pub const HXNU_SYS_GETTIMEOFDAY: u64 = 0x484e_0024;
pub const HXNU_SYS_GETRANDOM: u64 = 0x484e_0025;
pub const HXNU_SYS_RT_SIGACTION: u64 = 0x484e_0026;
pub const HXNU_SYS_RT_SIGPROCMASK: u64 = 0x484e_0027;
pub const HXNU_SYS_PREAD64: u64 = 0x484e_0028;
pub const HXNU_SYS_PWRITE64: u64 = 0x484e_0029;
pub const HXNU_SYS_READV: u64 = 0x484e_002a;
pub const HXNU_SYS_WRITEV: u64 = 0x484e_002b;
pub const HXNU_SYS_WAIT4: u64 = 0x484e_002c;
pub const HXNU_SYS_SETPGID: u64 = 0x484e_002d;
pub const HXNU_SYS_GETPGID: u64 = 0x484e_002e;
pub const HXNU_SYS_SETSID: u64 = 0x484e_002f;
pub const HXNU_SYS_GETSID: u64 = 0x484e_0030;
pub const HXNU_SYS_GETRLIMIT: u64 = 0x484e_0031;
pub const HXNU_SYS_SETRLIMIT: u64 = 0x484e_0032;
pub const HXNU_SYS_PRLIMIT64: u64 = 0x484e_0033;
pub const HXNU_SYS_PRCTL: u64 = 0x484e_0034;
pub const HXNU_SYS_SET_ROBUST_LIST: u64 = 0x484e_0035;
pub const HXNU_SYS_GET_ROBUST_LIST: u64 = 0x484e_0036;
pub const HXNU_SYS_RSEQ: u64 = 0x484e_0037;
pub const HXNU_SYS_EXIT_GROUP: u64 = 0x484e_00ff;

const HXNU_NATIVE_ABI_VERSION: i64 = 0x0001_0000;
const LINUX_CLOCK_REALTIME: i32 = 0;
const LINUX_CLOCK_MONOTONIC: i32 = 1;
const AT_FDCWD: i64 = -100;
const AT_EACCESS: u64 = 0x200;
const LINUX_TIOCGWINSZ: u64 = 0x5413;
const F_DUPFD: i32 = 0;
const F_GETFD: i32 = 1;
const F_SETFD: i32 = 2;
const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const FD_CLOEXEC: u32 = 1;

const F_OK: u64 = 0;
const X_OK: u64 = 1;
const W_OK: u64 = 2;
const R_OK: u64 = 4;

const O_ACCMODE: u64 = 0x3;
const O_RDONLY: u64 = 0;
const O_DIRECTORY: u64 = 0x10000;
const O_CLOEXEC: u64 = 0x80000;
const O_CREAT: u64 = 0x40;
const O_TRUNC: u64 = 0x200;
const O_APPEND: u64 = 0x400;

const PROT_NONE: u64 = 0;
const PROT_READ: u64 = 0x1;
const PROT_WRITE: u64 = 0x2;
const PROT_EXEC: u64 = 0x4;
const PROT_MASK: u64 = PROT_READ | PROT_WRITE | PROT_EXEC;

const MAP_SHARED: u64 = 0x01;
const MAP_PRIVATE: u64 = 0x02;
const MAP_FIXED: u64 = 0x10;
const MAP_ANONYMOUS: u64 = 0x20;

const GRND_NONBLOCK: u64 = 0x0001;
const GRND_RANDOM: u64 = 0x0002;
const GRND_INSECURE: u64 = 0x0004;
const GRND_MASK: u64 = GRND_NONBLOCK | GRND_RANDOM | GRND_INSECURE;

const RT_SIGSET_SIZE: usize = 8;
const SIG_BLOCK: i32 = 0;
const SIG_UNBLOCK: i32 = 1;
const SIG_SETMASK: i32 = 2;
const MAX_SIGNAL_NUMBER: u64 = 64;
const SIGKILL: u64 = 9;
const SIGSTOP: u64 = 19;
const WNOHANG: i32 = 1;
const MAX_IOVEC_COUNT: usize = 64;
const RLIM_INFINITY: u64 = u64::MAX;
const RLIMIT_CPU: u32 = 0;
const RLIMIT_FSIZE: u32 = 1;
const RLIMIT_DATA: u32 = 2;
const RLIMIT_STACK: u32 = 3;
const RLIMIT_CORE: u32 = 4;
const RLIMIT_RSS: u32 = 5;
const RLIMIT_NPROC: u32 = 6;
const RLIMIT_NOFILE: u32 = 7;
const RLIMIT_MEMLOCK: u32 = 8;
const RLIMIT_AS: u32 = 9;
const RLIMIT_LOCKS: u32 = 10;
const RLIMIT_SIGPENDING: u32 = 11;
const RLIMIT_MSGQUEUE: u32 = 12;
const RLIMIT_NICE: u32 = 13;
const RLIMIT_RTPRIO: u32 = 14;
const RLIMIT_RTTIME: u32 = 15;
const PR_GET_DUMPABLE: i32 = 3;
const PR_SET_DUMPABLE: i32 = 4;
const PR_SET_NAME: i32 = 15;
const PR_GET_NAME: i32 = 16;
const TASK_COMM_LEN: usize = 16;
const RSEQ_FLAG_UNREGISTER: u32 = 1;
const RSEQ_SIGNATURE: u32 = 0x5305_3053;

const SEEK_SET: i32 = 0;
const SEEK_CUR: i32 = 1;
const SEEK_END: i32 = 2;

const S_IFREG: u32 = 0o100000;
const S_IFDIR: u32 = 0o040000;
const S_IFCHR: u32 = 0o020000;
const MODE_REGULAR_READ_ONLY: u32 = 0o444;
const MODE_REGULAR_EXECUTABLE: u32 = 0o555;
const MODE_DIRECTORY: u32 = 0o555;
const MODE_CHARACTER_DEVICE: u32 = 0o666;
const DEFAULT_LINK_COUNT: u64 = 1;
const STAT_BLOCK_SIZE: i64 = 4096;
const STAT_SECTOR_SIZE: usize = 512;
const DT_UNKNOWN: u8 = 0;
const DT_CHR: u8 = 2;
const DT_DIR: u8 = 4;
const DT_REG: u8 = 8;

const MAX_WRITE_BYTES: usize = 16 * 1024;
const MAX_READ_BYTES: usize = 64 * 1024;
const MAX_PATH_BYTES: usize = 1024;
const MAX_OPEN_FILES: usize = 64;
const MMAP_PAGE_SIZE: usize = 4096;
const DEFAULT_PROCESS_BRK: usize = 0x4000_0000;
const DEFAULT_PROCESS_UMASK: u32 = 0o022;
const UMASK_MODE_MASK: u32 = 0o777;

const EPERM: i64 = 1;
const EBADF: i64 = 9;
const EIO: i64 = 5;
const EINVAL: i64 = 22;
const ENOSYS: i64 = 38;
const ERANGE: i64 = 34;
const ENOENT: i64 = 2;
const EACCES: i64 = 13;
const ENOTTY: i64 = 25;
const ENOTDIR: i64 = 20;
const EISDIR: i64 = 21;
const EMFILE: i64 = 24;
const ENOMEM: i64 = 12;
const EROFS: i64 = 30;
const ECHILD: i64 = 10;
const ESRCH: i64 = 3;

const STDOUT_FD: u64 = 1;
const STDERR_FD: u64 = 2;

#[derive(Copy, Clone)]
pub enum SyscallAbi {
    LinuxBootstrap,
    GhostBootstrap,
    HxnuNativeBootstrap,
}

impl SyscallAbi {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LinuxBootstrap => LINUX_ABI_NAME,
            Self::GhostBootstrap => GHOST_ABI_NAME,
            Self::HxnuNativeBootstrap => HXNU_ABI_NAME,
        }
    }
}

#[derive(Copy, Clone)]
pub enum SyscallAction {
    Continue,
    ExitGroup { status: i32 },
}

#[derive(Copy, Clone)]
pub struct SyscallOutcome {
    pub value: i64,
    pub action: SyscallAction,
}

impl SyscallOutcome {
    const fn success(value: i64) -> Self {
        Self {
            value,
            action: SyscallAction::Continue,
        }
    }

    const fn errno(errno: i64) -> Self {
        Self {
            value: -errno,
            action: SyscallAction::Continue,
        }
    }
}

#[derive(Copy, Clone)]
pub struct LinuxBootstrapProbe {
    pub write_result: i64,
    pub openat_result: i64,
    pub mmap_result: i64,
    pub mprotect_result: i64,
    pub munmap_result: i64,
    pub brk_result: i64,
    pub brk_set_result: i64,
    pub brk_restore_result: i64,
    pub nanosleep_result: i64,
    pub gettimeofday_result: i64,
    pub gettimeofday_seconds: i64,
    pub gettimeofday_microseconds: i64,
    pub getrandom_result: i64,
    pub getrandom_sample: u64,
    pub rt_sigaction_result: i64,
    pub rt_sigprocmask_result: i64,
    pub rt_sigmask_snapshot: u64,
    pub rt_sigold_handler: u64,
    pub pread64_result: i64,
    pub pwrite64_result: i64,
    pub readv_result: i64,
    pub writev_result: i64,
    pub wait4_result: i64,
    pub setpgid_result: i64,
    pub getpgid_result: i64,
    pub setsid_result: i64,
    pub getsid_result: i64,
    pub getrlimit_result: i64,
    pub setrlimit_result: i64,
    pub prlimit64_result: i64,
    pub prctl_set_name_result: i64,
    pub prctl_get_name_result: i64,
    pub prctl_set_dumpable_result: i64,
    pub prctl_get_dumpable_result: i64,
    pub set_robust_list_result: i64,
    pub get_robust_list_result: i64,
    pub rseq_register_result: i64,
    pub rseq_unregister_result: i64,
    pub ioctl_result: i64,
    pub access_result: i64,
    pub newfstatat_result: i64,
    pub faccessat_result: i64,
    pub faccessat2_result: i64,
    pub readlinkat_result: i64,
    pub dup_result: i64,
    pub dup2_result: i64,
    pub dup3_result: i64,
    pub fcntl_getfd_result: i64,
    pub fcntl_getfl_result: i64,
    pub getcwd_result: i64,
    pub chdir_result: i64,
    pub fchdir_result: i64,
    pub read_result: i64,
    pub fstat_result: i64,
    pub getdents64_result: i64,
    pub lseek_result: i64,
    pub close_result: i64,
    pub getpid_result: i64,
    pub getppid_result: i64,
    pub gettid_result: i64,
    pub umask_result: i64,
    pub umask_restore_result: i64,
    pub getuid_result: i64,
    pub getgid_result: i64,
    pub geteuid_result: i64,
    pub getegid_result: i64,
    pub set_tid_address_result: i64,
    pub clear_tid_snapshot: i64,
    pub sched_yield_result: i64,
    pub clock_gettime_result: i64,
    pub clock_seconds: i64,
    pub clock_nanoseconds: i64,
    pub uname_result: i64,
    pub exit_group_captured: bool,
    pub exit_group_status: i32,
    machine_bytes: [u8; 16],
    machine_len: usize,
}

impl LinuxBootstrapProbe {
    pub fn machine_str(&self) -> &str {
        machine_str(&self.machine_bytes, self.machine_len)
    }
}

#[derive(Copy, Clone)]
pub struct GhostBootstrapProbe {
    pub write_result: i64,
    pub open_result: i64,
    pub mmap_result: i64,
    pub mprotect_result: i64,
    pub munmap_result: i64,
    pub brk_result: i64,
    pub brk_set_result: i64,
    pub brk_restore_result: i64,
    pub nanosleep_result: i64,
    pub gettimeofday_result: i64,
    pub gettimeofday_seconds: i64,
    pub gettimeofday_microseconds: i64,
    pub getrandom_result: i64,
    pub getrandom_sample: u64,
    pub rt_sigaction_result: i64,
    pub rt_sigprocmask_result: i64,
    pub rt_sigmask_snapshot: u64,
    pub rt_sigold_handler: u64,
    pub pread64_result: i64,
    pub pwrite64_result: i64,
    pub readv_result: i64,
    pub writev_result: i64,
    pub wait4_result: i64,
    pub setpgid_result: i64,
    pub getpgid_result: i64,
    pub setsid_result: i64,
    pub getsid_result: i64,
    pub getrlimit_result: i64,
    pub setrlimit_result: i64,
    pub prlimit64_result: i64,
    pub prctl_set_name_result: i64,
    pub prctl_get_name_result: i64,
    pub prctl_set_dumpable_result: i64,
    pub prctl_get_dumpable_result: i64,
    pub set_robust_list_result: i64,
    pub get_robust_list_result: i64,
    pub rseq_register_result: i64,
    pub rseq_unregister_result: i64,
    pub ioctl_result: i64,
    pub access_result: i64,
    pub stat_result: i64,
    pub readlink_result: i64,
    pub dup_result: i64,
    pub dup2_result: i64,
    pub dup3_result: i64,
    pub fcntl_getfd_result: i64,
    pub fcntl_getfl_result: i64,
    pub getcwd_result: i64,
    pub chdir_result: i64,
    pub fchdir_result: i64,
    pub read_result: i64,
    pub fstat_result: i64,
    pub getdents_result: i64,
    pub seek_result: i64,
    pub close_result: i64,
    pub getpid_result: i64,
    pub getppid_result: i64,
    pub gettid_result: i64,
    pub umask_result: i64,
    pub umask_restore_result: i64,
    pub getuid_result: i64,
    pub getgid_result: i64,
    pub geteuid_result: i64,
    pub getegid_result: i64,
    pub set_tid_address_result: i64,
    pub clear_tid_snapshot: i64,
    pub yield_result: i64,
    pub uptime_result: i64,
    pub uname_result: i64,
    pub exit_group_captured: bool,
    pub exit_group_status: i32,
    machine_bytes: [u8; 16],
    machine_len: usize,
}

impl GhostBootstrapProbe {
    pub fn machine_str(&self) -> &str {
        machine_str(&self.machine_bytes, self.machine_len)
    }
}

#[derive(Copy, Clone)]
pub struct HxnuBootstrapProbe {
    pub write_result: i64,
    pub open_result: i64,
    pub mmap_result: i64,
    pub mprotect_result: i64,
    pub munmap_result: i64,
    pub brk_result: i64,
    pub brk_set_result: i64,
    pub brk_restore_result: i64,
    pub nanosleep_result: i64,
    pub gettimeofday_result: i64,
    pub gettimeofday_seconds: i64,
    pub gettimeofday_microseconds: i64,
    pub getrandom_result: i64,
    pub getrandom_sample: u64,
    pub rt_sigaction_result: i64,
    pub rt_sigprocmask_result: i64,
    pub rt_sigmask_snapshot: u64,
    pub rt_sigold_handler: u64,
    pub pread64_result: i64,
    pub pwrite64_result: i64,
    pub readv_result: i64,
    pub writev_result: i64,
    pub wait4_result: i64,
    pub setpgid_result: i64,
    pub getpgid_result: i64,
    pub setsid_result: i64,
    pub getsid_result: i64,
    pub getrlimit_result: i64,
    pub setrlimit_result: i64,
    pub prlimit64_result: i64,
    pub prctl_set_name_result: i64,
    pub prctl_get_name_result: i64,
    pub prctl_set_dumpable_result: i64,
    pub prctl_get_dumpable_result: i64,
    pub set_robust_list_result: i64,
    pub get_robust_list_result: i64,
    pub rseq_register_result: i64,
    pub rseq_unregister_result: i64,
    pub ioctl_result: i64,
    pub access_result: i64,
    pub stat_result: i64,
    pub readlink_result: i64,
    pub dup_result: i64,
    pub dup2_result: i64,
    pub dup3_result: i64,
    pub fcntl_getfd_result: i64,
    pub fcntl_getfl_result: i64,
    pub getcwd_result: i64,
    pub chdir_result: i64,
    pub fchdir_result: i64,
    pub read_result: i64,
    pub fstat_result: i64,
    pub getdents_result: i64,
    pub seek_result: i64,
    pub close_result: i64,
    pub process_self_result: i64,
    pub process_parent_result: i64,
    pub thread_self_result: i64,
    pub umask_result: i64,
    pub umask_restore_result: i64,
    pub getuid_result: i64,
    pub getgid_result: i64,
    pub geteuid_result: i64,
    pub getegid_result: i64,
    pub set_tid_address_result: i64,
    pub clear_tid_snapshot: i64,
    pub sched_yield_result: i64,
    pub uptime_result: i64,
    pub abi_version_result: i64,
    pub exit_group_captured: bool,
    pub exit_group_status: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxTimespec {
    tv_sec: i64,
    tv_nsec: i64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxKernelSigAction {
    handler: u64,
    flags: u64,
    restorer: u64,
    mask: u64,
}

impl LinuxKernelSigAction {
    const fn empty() -> Self {
        Self {
            handler: 0,
            flags: 0,
            restorer: 0,
            mask: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxIovec {
    iov_base: u64,
    iov_len: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxTimeval {
    tv_sec: i64,
    tv_usec: i64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxRlimit64 {
    rlim_cur: u64,
    rlim_max: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxRobustListHead {
    list_next: u64,
    futex_offset: i64,
    list_op_pending: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxRseqArea {
    cpu_id_start: u32,
    cpu_id: u32,
    rseq_cs: u64,
    flags: u32,
    _reserved: [u8; 12],
}

impl LinuxRseqArea {
    const fn empty() -> Self {
        Self {
            cpu_id_start: 0,
            cpu_id: 0,
            rseq_cs: 0,
            flags: 0,
            _reserved: [0; 12],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxWinsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxStat {
    st_dev: u64,
    st_ino: u64,
    st_nlink: u64,
    st_mode: u32,
    st_uid: u32,
    st_gid: u32,
    __pad0: u32,
    st_rdev: u64,
    st_size: i64,
    st_blksize: i64,
    st_blocks: i64,
    st_atime: i64,
    st_atime_nsec: i64,
    st_mtime: i64,
    st_mtime_nsec: i64,
    st_ctime: i64,
    st_ctime_nsec: i64,
    __unused: [i64; 3],
}

impl LinuxStat {
    const fn empty() -> Self {
        Self {
            st_dev: 0,
            st_ino: 0,
            st_nlink: 0,
            st_mode: 0,
            st_uid: 0,
            st_gid: 0,
            __pad0: 0,
            st_rdev: 0,
            st_size: 0,
            st_blksize: 0,
            st_blocks: 0,
            st_atime: 0,
            st_atime_nsec: 0,
            st_mtime: 0,
            st_mtime_nsec: 0,
            st_ctime: 0,
            st_ctime_nsec: 0,
            __unused: [0; 3],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct LinuxUtsName {
    sysname: [u8; 65],
    nodename: [u8; 65],
    release: [u8; 65],
    version: [u8; 65],
    machine: [u8; 65],
    domainname: [u8; 65],
}

impl LinuxUtsName {
    const fn new() -> Self {
        Self {
            sysname: [0; 65],
            nodename: [0; 65],
            release: [0; 65],
            version: [0; 65],
            machine: [0; 65],
            domainname: [0; 65],
        }
    }
}

struct OpenFile {
    fd: i32,
    fd_flags: u32,
    owner_process_id: u64,
    mount: VfsMountKind,
    kind: VfsNodeKind,
    executable: bool,
    path: String,
    offset: usize,
    content: Vec<u8>,
}

struct FdTable {
    next_fd: i32,
    files: Vec<OpenFile>,
}

impl FdTable {
    fn new() -> Self {
        Self {
            next_fd: 3,
            files: Vec::new(),
        }
    }
}

struct GlobalFdTable(UnsafeCell<Option<FdTable>>);

unsafe impl Sync for GlobalFdTable {}

impl GlobalFdTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<FdTable> {
        self.0.get()
    }
}

static FD_TABLE: GlobalFdTable = GlobalFdTable::new();

struct ProcessCwd {
    process_id: u64,
    path: String,
}

struct GlobalCwdTable(UnsafeCell<Option<Vec<ProcessCwd>>>);

unsafe impl Sync for GlobalCwdTable {}

impl GlobalCwdTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<Vec<ProcessCwd>> {
        self.0.get()
    }
}

static CWD_TABLE: GlobalCwdTable = GlobalCwdTable::new();

struct ProcessUmask {
    process_id: u64,
    mask: u32,
}

struct GlobalUmaskTable(UnsafeCell<Option<Vec<ProcessUmask>>>);

unsafe impl Sync for GlobalUmaskTable {}

impl GlobalUmaskTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<Vec<ProcessUmask>> {
        self.0.get()
    }
}

static UMASK_TABLE: GlobalUmaskTable = GlobalUmaskTable::new();

struct ProcessClearTidAddress {
    process_id: u64,
    address: usize,
}

struct GlobalClearTidTable(UnsafeCell<Option<Vec<ProcessClearTidAddress>>>);

unsafe impl Sync for GlobalClearTidTable {}

impl GlobalClearTidTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<Vec<ProcessClearTidAddress>> {
        self.0.get()
    }
}

static CLEAR_TID_TABLE: GlobalClearTidTable = GlobalClearTidTable::new();

struct ProcessMapping {
    process_id: u64,
    base: usize,
    len: usize,
    prot: u64,
}

struct GlobalMappingTable(UnsafeCell<Option<Vec<ProcessMapping>>>);

unsafe impl Sync for GlobalMappingTable {}

impl GlobalMappingTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<Vec<ProcessMapping>> {
        self.0.get()
    }
}

static MAPPING_TABLE: GlobalMappingTable = GlobalMappingTable::new();

struct ProcessBrkState {
    process_id: u64,
    current_break: usize,
}

struct GlobalBrkTable(UnsafeCell<Option<Vec<ProcessBrkState>>>);

unsafe impl Sync for GlobalBrkTable {}

impl GlobalBrkTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<Vec<ProcessBrkState>> {
        self.0.get()
    }
}

static BRK_TABLE: GlobalBrkTable = GlobalBrkTable::new();

struct ProcessSignalMask {
    process_id: u64,
    mask: u64,
}

struct GlobalSignalMaskTable(UnsafeCell<Option<Vec<ProcessSignalMask>>>);

unsafe impl Sync for GlobalSignalMaskTable {}

impl GlobalSignalMaskTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<Vec<ProcessSignalMask>> {
        self.0.get()
    }
}

static SIGNAL_MASK_TABLE: GlobalSignalMaskTable = GlobalSignalMaskTable::new();

struct ProcessSignalAction {
    process_id: u64,
    signum: u8,
    action: LinuxKernelSigAction,
}

struct GlobalSignalActionTable(UnsafeCell<Option<Vec<ProcessSignalAction>>>);

unsafe impl Sync for GlobalSignalActionTable {}

impl GlobalSignalActionTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<Vec<ProcessSignalAction>> {
        self.0.get()
    }
}

static SIGNAL_ACTION_TABLE: GlobalSignalActionTable = GlobalSignalActionTable::new();

struct ProcessGroupState {
    process_id: u64,
    process_group_id: u64,
    session_id: u64,
}

struct GlobalProcessGroupTable(UnsafeCell<Option<Vec<ProcessGroupState>>>);

unsafe impl Sync for GlobalProcessGroupTable {}

impl GlobalProcessGroupTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<Vec<ProcessGroupState>> {
        self.0.get()
    }
}

static PROCESS_GROUP_TABLE: GlobalProcessGroupTable = GlobalProcessGroupTable::new();

struct ProcessRlimitState {
    process_id: u64,
    resource: u32,
    limits: LinuxRlimit64,
}

struct GlobalRlimitTable(UnsafeCell<Option<Vec<ProcessRlimitState>>>);

unsafe impl Sync for GlobalRlimitTable {}

impl GlobalRlimitTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<Vec<ProcessRlimitState>> {
        self.0.get()
    }
}

static RLIMIT_TABLE: GlobalRlimitTable = GlobalRlimitTable::new();

struct ProcessPrctlState {
    process_id: u64,
    name: [u8; TASK_COMM_LEN],
    dumpable: i32,
}

struct GlobalPrctlTable(UnsafeCell<Option<Vec<ProcessPrctlState>>>);

unsafe impl Sync for GlobalPrctlTable {}

impl GlobalPrctlTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<Vec<ProcessPrctlState>> {
        self.0.get()
    }
}

static PRCTL_TABLE: GlobalPrctlTable = GlobalPrctlTable::new();

struct ProcessRobustListState {
    process_id: u64,
    head: usize,
    len: usize,
}

struct GlobalRobustListTable(UnsafeCell<Option<Vec<ProcessRobustListState>>>);

unsafe impl Sync for GlobalRobustListTable {}

impl GlobalRobustListTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<Vec<ProcessRobustListState>> {
        self.0.get()
    }
}

static ROBUST_LIST_TABLE: GlobalRobustListTable = GlobalRobustListTable::new();

struct ProcessRseqState {
    process_id: u64,
    address: usize,
    length: u32,
    signature: u32,
    registered: bool,
}

struct GlobalRseqTable(UnsafeCell<Option<Vec<ProcessRseqState>>>);

unsafe impl Sync for GlobalRseqTable {}

impl GlobalRseqTable {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<Vec<ProcessRseqState>> {
        self.0.get()
    }
}

static RSEQ_TABLE: GlobalRseqTable = GlobalRseqTable::new();

pub fn dispatch(abi: SyscallAbi, number: u64, args: [u64; 6]) -> SyscallOutcome {
    match abi {
        SyscallAbi::LinuxBootstrap => dispatch_linux_bootstrap(number, args),
        SyscallAbi::GhostBootstrap => dispatch_ghost_bootstrap(number, args),
        SyscallAbi::HxnuNativeBootstrap => dispatch_hxnu_bootstrap(number, args),
    }
}

pub fn dispatch_linux_bootstrap(number: u64, args: [u64; 6]) -> SyscallOutcome {
    match number {
        LINUX_SYS_READ => read_from_fd(args),
        LINUX_SYS_WRITE => write_with_fd(args),
        LINUX_SYS_CLOSE => close_fd(args),
        LINUX_SYS_DUP => dup_fd(args),
        LINUX_SYS_FSTAT => fstat_fd(args),
        LINUX_SYS_MMAP => linux_mmap(args),
        LINUX_SYS_MPROTECT => process_mprotect(args),
        LINUX_SYS_MUNMAP => process_munmap(args),
        LINUX_SYS_BRK => process_brk(args),
        LINUX_SYS_RT_SIGACTION => process_rt_sigaction(args),
        LINUX_SYS_RT_SIGPROCMASK => process_rt_sigprocmask(args),
        LINUX_SYS_PREAD64 => process_pread64(args),
        LINUX_SYS_PWRITE64 => process_pwrite64(args),
        LINUX_SYS_READV => process_readv(args),
        LINUX_SYS_WRITEV => process_writev(args),
        LINUX_SYS_NANOSLEEP => process_nanosleep(args),
        LINUX_SYS_GETDENTS64 => getdents_fd(args),
        LINUX_SYS_IOCTL => ioctl_fd(args),
        LINUX_SYS_LSEEK => seek_fd(args),
        LINUX_SYS_ACCESS => access_path_at(AT_FDCWD, args[0] as usize, args[1], 0),
        LINUX_SYS_FCNTL => fcntl_fd(args),
        LINUX_SYS_DUP2 => dup2_fd(args),
        LINUX_SYS_GETCWD => linux_getcwd(args),
        LINUX_SYS_CHDIR => linux_chdir(args),
        LINUX_SYS_FCHDIR => linux_fchdir(args),
        LINUX_SYS_OPENAT => linux_openat(args),
        LINUX_SYS_NEWFSTATAT => linux_newfstatat(args),
        LINUX_SYS_READLINKAT => linux_readlinkat(args),
        LINUX_SYS_FACCESSAT => linux_faccessat(args),
        LINUX_SYS_FACCESSAT2 => linux_faccessat2(args),
        LINUX_SYS_DUP3 => dup3_fd(args),
        LINUX_SYS_SCHED_YIELD => SyscallOutcome::success(0),
        LINUX_SYS_GETPID => process_id(),
        LINUX_SYS_WAIT4 => process_wait4(args),
        LINUX_SYS_GETPPID => process_parent_id(),
        LINUX_SYS_GETTID => thread_id(),
        LINUX_SYS_SETPGID => process_setpgid(args),
        LINUX_SYS_GETPGID => process_getpgid(args),
        LINUX_SYS_SETSID => process_setsid(),
        LINUX_SYS_GETSID => process_getsid(args),
        LINUX_SYS_GETRLIMIT => process_getrlimit(args),
        LINUX_SYS_SETRLIMIT => process_setrlimit(args),
        LINUX_SYS_PRLIMIT64 => process_prlimit64(args),
        LINUX_SYS_PRCTL => process_prctl(args),
        LINUX_SYS_SET_ROBUST_LIST => process_set_robust_list(args),
        LINUX_SYS_GET_ROBUST_LIST => process_get_robust_list(args),
        LINUX_SYS_RSEQ => process_rseq(args),
        LINUX_SYS_UMASK => process_umask(args),
        LINUX_SYS_GETUID | LINUX_SYS_GETEUID => user_id(),
        LINUX_SYS_GETGID | LINUX_SYS_GETEGID => group_id(),
        LINUX_SYS_SET_TID_ADDRESS => set_tid_address(args),
        LINUX_SYS_GETTIMEOFDAY => process_gettimeofday(args),
        LINUX_SYS_CLOCK_GETTIME => linux_clock_gettime(args),
        LINUX_SYS_GETRANDOM => process_getrandom(args),
        LINUX_SYS_UNAME => linux_uname(args),
        LINUX_SYS_EXIT | LINUX_SYS_EXIT_GROUP => exit_group(args),
        _ => SyscallOutcome::errno(ENOSYS),
    }
}

pub fn dispatch_ghost_bootstrap(number: u64, args: [u64; 6]) -> SyscallOutcome {
    match number {
        GHOST_SYS_WRITE => write_with_fd(args),
        GHOST_SYS_OPEN => open_path_at(AT_FDCWD, args[0] as usize, args[1]),
        GHOST_SYS_READ => read_from_fd(args),
        GHOST_SYS_CLOSE => close_fd(args),
        GHOST_SYS_MMAP => process_mmap(args),
        GHOST_SYS_MPROTECT => process_mprotect(args),
        GHOST_SYS_MUNMAP => process_munmap(args),
        GHOST_SYS_BRK => process_brk(args),
        GHOST_SYS_RT_SIGACTION => process_rt_sigaction(args),
        GHOST_SYS_RT_SIGPROCMASK => process_rt_sigprocmask(args),
        GHOST_SYS_PREAD64 => process_pread64(args),
        GHOST_SYS_PWRITE64 => process_pwrite64(args),
        GHOST_SYS_READV => process_readv(args),
        GHOST_SYS_WRITEV => process_writev(args),
        GHOST_SYS_NANOSLEEP => process_nanosleep(args),
        GHOST_SYS_DUP => dup_fd(args),
        GHOST_SYS_DUP2 => dup2_fd(args),
        GHOST_SYS_DUP3 => dup3_fd(args),
        GHOST_SYS_FCNTL => fcntl_fd(args),
        GHOST_SYS_GETCWD => process_getcwd(args),
        GHOST_SYS_CHDIR => process_chdir(args),
        GHOST_SYS_FCHDIR => process_fchdir(args),
        GHOST_SYS_FSTAT => fstat_fd(args),
        GHOST_SYS_GETDENTS => getdents_fd(args),
        GHOST_SYS_IOCTL => ioctl_fd(args),
        GHOST_SYS_STAT => stat_path_at(AT_FDCWD, args[0] as usize, args[1], 0),
        GHOST_SYS_READLINK => readlink_path_at(AT_FDCWD, args[0] as usize, args[1] as usize, args[2]),
        GHOST_SYS_ACCESS => access_path_at(AT_FDCWD, args[0] as usize, args[1], 0),
        GHOST_SYS_SEEK => seek_fd(args),
        GHOST_SYS_YIELD => SyscallOutcome::success(0),
        GHOST_SYS_GETPID => process_id(),
        GHOST_SYS_WAIT4 => process_wait4(args),
        GHOST_SYS_GETPPID => process_parent_id(),
        GHOST_SYS_GETTID => thread_id(),
        GHOST_SYS_SETPGID => process_setpgid(args),
        GHOST_SYS_GETPGID => process_getpgid(args),
        GHOST_SYS_SETSID => process_setsid(),
        GHOST_SYS_GETSID => process_getsid(args),
        GHOST_SYS_GETRLIMIT => process_getrlimit(args),
        GHOST_SYS_SETRLIMIT => process_setrlimit(args),
        GHOST_SYS_PRLIMIT64 => process_prlimit64(args),
        GHOST_SYS_PRCTL => process_prctl(args),
        GHOST_SYS_SET_ROBUST_LIST => process_set_robust_list(args),
        GHOST_SYS_GET_ROBUST_LIST => process_get_robust_list(args),
        GHOST_SYS_RSEQ => process_rseq(args),
        GHOST_SYS_UMASK => process_umask(args),
        GHOST_SYS_GETUID | GHOST_SYS_GETEUID => user_id(),
        GHOST_SYS_GETGID | GHOST_SYS_GETEGID => group_id(),
        GHOST_SYS_SET_TID_ADDRESS => set_tid_address(args),
        GHOST_SYS_GETTIMEOFDAY => process_gettimeofday(args),
        GHOST_SYS_GETRANDOM => process_getrandom(args),
        GHOST_SYS_UPTIME_NSEC => uptime_ns(),
        GHOST_SYS_UNAME => ghost_uname(args),
        GHOST_SYS_EXIT_GROUP => exit_group(args),
        _ => SyscallOutcome::errno(ENOSYS),
    }
}

pub fn dispatch_hxnu_bootstrap(number: u64, args: [u64; 6]) -> SyscallOutcome {
    match number {
        HXNU_SYS_LOG_WRITE => write_without_fd(args),
        HXNU_SYS_OPEN => open_path_at(AT_FDCWD, args[0] as usize, args[1]),
        HXNU_SYS_READ => read_from_fd(args),
        HXNU_SYS_CLOSE => close_fd(args),
        HXNU_SYS_MMAP => process_mmap(args),
        HXNU_SYS_MPROTECT => process_mprotect(args),
        HXNU_SYS_MUNMAP => process_munmap(args),
        HXNU_SYS_BRK => process_brk(args),
        HXNU_SYS_RT_SIGACTION => process_rt_sigaction(args),
        HXNU_SYS_RT_SIGPROCMASK => process_rt_sigprocmask(args),
        HXNU_SYS_PREAD64 => process_pread64(args),
        HXNU_SYS_PWRITE64 => process_pwrite64(args),
        HXNU_SYS_READV => process_readv(args),
        HXNU_SYS_WRITEV => process_writev(args),
        HXNU_SYS_NANOSLEEP => process_nanosleep(args),
        HXNU_SYS_DUP => dup_fd(args),
        HXNU_SYS_DUP2 => dup2_fd(args),
        HXNU_SYS_DUP3 => dup3_fd(args),
        HXNU_SYS_FCNTL => fcntl_fd(args),
        HXNU_SYS_GETCWD => process_getcwd(args),
        HXNU_SYS_CHDIR => process_chdir(args),
        HXNU_SYS_FCHDIR => process_fchdir(args),
        HXNU_SYS_FSTAT => fstat_fd(args),
        HXNU_SYS_GETDENTS => getdents_fd(args),
        HXNU_SYS_IOCTL => ioctl_fd(args),
        HXNU_SYS_STAT => stat_path_at(AT_FDCWD, args[0] as usize, args[1], 0),
        HXNU_SYS_READLINK => readlink_path_at(AT_FDCWD, args[0] as usize, args[1] as usize, args[2]),
        HXNU_SYS_ACCESS => access_path_at(AT_FDCWD, args[0] as usize, args[1], 0),
        HXNU_SYS_SEEK => seek_fd(args),
        HXNU_SYS_THREAD_SELF => thread_id(),
        HXNU_SYS_PROCESS_SELF => process_id(),
        HXNU_SYS_WAIT4 => process_wait4(args),
        HXNU_SYS_PROCESS_PARENT => process_parent_id(),
        HXNU_SYS_SETPGID => process_setpgid(args),
        HXNU_SYS_GETPGID => process_getpgid(args),
        HXNU_SYS_SETSID => process_setsid(),
        HXNU_SYS_GETSID => process_getsid(args),
        HXNU_SYS_GETRLIMIT => process_getrlimit(args),
        HXNU_SYS_SETRLIMIT => process_setrlimit(args),
        HXNU_SYS_PRLIMIT64 => process_prlimit64(args),
        HXNU_SYS_PRCTL => process_prctl(args),
        HXNU_SYS_SET_ROBUST_LIST => process_set_robust_list(args),
        HXNU_SYS_GET_ROBUST_LIST => process_get_robust_list(args),
        HXNU_SYS_RSEQ => process_rseq(args),
        HXNU_SYS_UMASK => process_umask(args),
        HXNU_SYS_GETUID | HXNU_SYS_GETEUID => user_id(),
        HXNU_SYS_GETGID | HXNU_SYS_GETEGID => group_id(),
        HXNU_SYS_SET_TID_ADDRESS => set_tid_address(args),
        HXNU_SYS_GETTIMEOFDAY => process_gettimeofday(args),
        HXNU_SYS_GETRANDOM => process_getrandom(args),
        HXNU_SYS_UPTIME_NSEC => uptime_ns(),
        HXNU_SYS_SCHED_YIELD => SyscallOutcome::success(0),
        HXNU_SYS_ABI_VERSION => SyscallOutcome::success(HXNU_NATIVE_ABI_VERSION),
        HXNU_SYS_EXIT_GROUP => exit_group(args),
        _ => SyscallOutcome::errno(ENOSYS),
    }
}

pub fn run_linux_bootstrap_probe() -> LinuxBootstrapProbe {
    static WRITE_SMOKE: &[u8] = b"HXNU: linux syscall write() compatibility smoke\n";
    static WRITEV_SMOKE_A: &[u8] = b"HXNU: linux writev ";
    static WRITEV_SMOKE_B: &[u8] = b"compatibility smoke\n";
    static OPEN_PATH: &[u8] = b"/proc/version\0";
    static OPEN_DIR_PATH: &[u8] = b"/proc\0";
    static ROOT_PATH: &[u8] = b"/\0";
    static READLINK_PATH: &[u8] = b"/proc/self/exe\0";
    let abi = SyscallAbi::LinuxBootstrap;

    let write_result = dispatch(
        abi,
        LINUX_SYS_WRITE,
        [
            STDOUT_FD,
            WRITE_SMOKE.as_ptr() as u64,
            WRITE_SMOKE.len() as u64,
            0,
            0,
            0,
        ],
    )
    .value;

    let openat_result = dispatch(
        abi,
        LINUX_SYS_OPENAT,
        [AT_FDCWD as u64, OPEN_PATH.as_ptr() as u64, 0, 0, 0, 0],
    )
    .value;
    let mmap_result = dispatch(
        abi,
        LINUX_SYS_MMAP,
        [
            0,
            MMAP_PAGE_SIZE as u64,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            u64::MAX,
            0,
        ],
    )
    .value;
    let mut mprotect_result = -EINVAL;
    let mut munmap_result = -EINVAL;
    if mmap_result >= 0 {
        let address = mmap_result as u64;
        mprotect_result = dispatch(
            abi,
            LINUX_SYS_MPROTECT,
            [address, MMAP_PAGE_SIZE as u64, PROT_READ, 0, 0, 0],
        )
        .value;
        munmap_result = dispatch(abi, LINUX_SYS_MUNMAP, [address, MMAP_PAGE_SIZE as u64, 0, 0, 0, 0]).value;
    }
    let brk_result = dispatch(abi, LINUX_SYS_BRK, [0, 0, 0, 0, 0, 0]).value;
    let brk_set_target = if brk_result >= 0 {
        (brk_result as u64).saturating_add(MMAP_PAGE_SIZE as u64)
    } else {
        (DEFAULT_PROCESS_BRK as u64).saturating_add(MMAP_PAGE_SIZE as u64)
    };
    let brk_set_result = dispatch(abi, LINUX_SYS_BRK, [brk_set_target, 0, 0, 0, 0, 0]).value;
    let brk_restore_target = if brk_result >= 0 {
        brk_result as u64
    } else {
        DEFAULT_PROCESS_BRK as u64
    };
    let brk_restore_result = dispatch(abi, LINUX_SYS_BRK, [brk_restore_target, 0, 0, 0, 0, 0]).value;
    let nanosleep_request = LinuxTimespec {
        tv_sec: 0,
        tv_nsec: 500_000,
    };
    let nanosleep_result = dispatch(
        abi,
        LINUX_SYS_NANOSLEEP,
        [(&nanosleep_request as *const LinuxTimespec) as u64, 0, 0, 0, 0, 0],
    )
    .value;
    let mut timeval = LinuxTimeval { tv_sec: 0, tv_usec: 0 };
    let gettimeofday_result = dispatch(
        abi,
        LINUX_SYS_GETTIMEOFDAY,
        [(&mut timeval as *mut LinuxTimeval) as u64, 0, 0, 0, 0, 0],
    )
    .value;
    let mut random_buffer = [0u8; 16];
    let getrandom_result = dispatch(
        abi,
        LINUX_SYS_GETRANDOM,
        [
            random_buffer.as_mut_ptr() as u64,
            random_buffer.len() as u64,
            GRND_NONBLOCK,
            0,
            0,
            0,
        ],
    )
    .value;
    let getrandom_sample = sample_random_u64(&random_buffer);
    let signal_set = 1u64 << 1;
    let mut previous_signal_mask = 0u64;
    let rt_sigprocmask_result = dispatch(
        abi,
        LINUX_SYS_RT_SIGPROCMASK,
        [
            SIG_BLOCK as u64,
            (&signal_set as *const u64) as u64,
            (&mut previous_signal_mask as *mut u64) as u64,
            RT_SIGSET_SIZE as u64,
            0,
            0,
        ],
    )
    .value;
    let action = LinuxKernelSigAction {
        handler: 0x10,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    let mut old_action = LinuxKernelSigAction::empty();
    let rt_sigaction_result = dispatch(
        abi,
        LINUX_SYS_RT_SIGACTION,
        [
            10,
            (&action as *const LinuxKernelSigAction) as u64,
            (&mut old_action as *mut LinuxKernelSigAction) as u64,
            RT_SIGSET_SIZE as u64,
            0,
            0,
        ],
    )
    .value;
    let rt_sigmask_snapshot = previous_signal_mask;
    let rt_sigold_handler = old_action.handler;
    let wait4_result = dispatch(abi, LINUX_SYS_WAIT4, [u64::MAX, 0, WNOHANG as u64, 0, 0, 0]).value;
    let setpgid_result = dispatch(abi, LINUX_SYS_SETPGID, [0, 0, 0, 0, 0, 0]).value;
    let getpgid_result = dispatch(abi, LINUX_SYS_GETPGID, [0, 0, 0, 0, 0, 0]).value;
    let setsid_result = dispatch(abi, LINUX_SYS_SETSID, [0, 0, 0, 0, 0, 0]).value;
    let getsid_result = dispatch(abi, LINUX_SYS_GETSID, [0, 0, 0, 0, 0, 0]).value;
    let mut nofile_limit = LinuxRlimit64 {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let getrlimit_result = dispatch(
        abi,
        LINUX_SYS_GETRLIMIT,
        [
            RLIMIT_NOFILE as u64,
            (&mut nofile_limit as *mut LinuxRlimit64) as u64,
            0,
            0,
            0,
            0,
        ],
    )
    .value;
    let setrlimit_result = dispatch(
        abi,
        LINUX_SYS_SETRLIMIT,
        [
            RLIMIT_NOFILE as u64,
            (&nofile_limit as *const LinuxRlimit64) as u64,
            0,
            0,
            0,
            0,
        ],
    )
    .value;
    let mut prlimit_old = LinuxRlimit64 {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let prlimit64_result = dispatch(
        abi,
        LINUX_SYS_PRLIMIT64,
        [
            0,
            RLIMIT_NOFILE as u64,
            0,
            (&mut prlimit_old as *mut LinuxRlimit64) as u64,
            0,
            0,
        ],
    )
    .value;
    let prctl_name = b"linux-bootstrap\0";
    let prctl_set_name_result = dispatch(
        abi,
        LINUX_SYS_PRCTL,
        [PR_SET_NAME as u64, prctl_name.as_ptr() as u64, 0, 0, 0, 0],
    )
    .value;
    let mut prctl_name_readback = [0u8; TASK_COMM_LEN];
    let prctl_get_name_result = dispatch(
        abi,
        LINUX_SYS_PRCTL,
        [PR_GET_NAME as u64, prctl_name_readback.as_mut_ptr() as u64, 0, 0, 0, 0],
    )
    .value;
    let prctl_set_dumpable_result = dispatch(
        abi,
        LINUX_SYS_PRCTL,
        [PR_SET_DUMPABLE as u64, 1, 0, 0, 0, 0],
    )
    .value;
    let prctl_get_dumpable_result = dispatch(abi, LINUX_SYS_PRCTL, [PR_GET_DUMPABLE as u64, 0, 0, 0, 0, 0]).value;
    let robust_head = LinuxRobustListHead {
        list_next: 0,
        futex_offset: 0,
        list_op_pending: 0,
    };
    let set_robust_list_result = dispatch(
        abi,
        LINUX_SYS_SET_ROBUST_LIST,
        [
            (&robust_head as *const LinuxRobustListHead) as u64,
            size_of::<LinuxRobustListHead>() as u64,
            0,
            0,
            0,
            0,
        ],
    )
    .value;
    let mut robust_head_readback = 0u64;
    let mut robust_len_readback = 0usize;
    let get_robust_list_result = dispatch(
        abi,
        LINUX_SYS_GET_ROBUST_LIST,
        [
            0,
            (&mut robust_head_readback as *mut u64) as u64,
            (&mut robust_len_readback as *mut usize) as u64,
            0,
            0,
            0,
        ],
    )
    .value;
    let rseq_area = LinuxRseqArea::empty();
    let rseq_register_result = dispatch(
        abi,
        LINUX_SYS_RSEQ,
        [
            (&rseq_area as *const LinuxRseqArea) as u64,
            size_of::<LinuxRseqArea>() as u64,
            0,
            RSEQ_SIGNATURE as u64,
            0,
            0,
        ],
    )
    .value;
    let rseq_unregister_result = dispatch(
        abi,
        LINUX_SYS_RSEQ,
        [
            (&rseq_area as *const LinuxRseqArea) as u64,
            size_of::<LinuxRseqArea>() as u64,
            RSEQ_FLAG_UNREGISTER as u64,
            RSEQ_SIGNATURE as u64,
            0,
            0,
        ],
    )
    .value;
    let writev_iov = [
        LinuxIovec {
            iov_base: WRITEV_SMOKE_A.as_ptr() as u64,
            iov_len: WRITEV_SMOKE_A.len() as u64,
        },
        LinuxIovec {
            iov_base: WRITEV_SMOKE_B.as_ptr() as u64,
            iov_len: WRITEV_SMOKE_B.len() as u64,
        },
    ];
    let writev_result = dispatch(
        abi,
        LINUX_SYS_WRITEV,
        [STDOUT_FD, writev_iov.as_ptr() as u64, writev_iov.len() as u64, 0, 0, 0],
    )
    .value;
    let mut winsize = LinuxWinsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let ioctl_result = dispatch(
        abi,
        LINUX_SYS_IOCTL,
        [STDOUT_FD, LINUX_TIOCGWINSZ, (&mut winsize as *mut LinuxWinsize) as u64, 0, 0, 0],
    )
    .value;
    let access_result = dispatch(abi, LINUX_SYS_ACCESS, [OPEN_PATH.as_ptr() as u64, R_OK, 0, 0, 0, 0]).value;
    let mut stat = LinuxStat::empty();
    let newfstatat_result = dispatch(
        abi,
        LINUX_SYS_NEWFSTATAT,
        [
            AT_FDCWD as u64,
            OPEN_PATH.as_ptr() as u64,
            (&mut stat as *mut LinuxStat) as u64,
            0,
            0,
            0,
        ],
    )
    .value;
    let faccessat_result = dispatch(
        abi,
        LINUX_SYS_FACCESSAT,
        [AT_FDCWD as u64, OPEN_PATH.as_ptr() as u64, R_OK, 0, 0, 0],
    )
    .value;
    let faccessat2_result = dispatch(
        abi,
        LINUX_SYS_FACCESSAT2,
        [AT_FDCWD as u64, OPEN_PATH.as_ptr() as u64, R_OK, 0, 0, 0],
    )
    .value;
    let mut cwd_buffer = [0u8; 128];
    let getcwd_result = dispatch(
        abi,
        LINUX_SYS_GETCWD,
        [cwd_buffer.as_mut_ptr() as u64, cwd_buffer.len() as u64, 0, 0, 0, 0],
    )
    .value;
    let chdir_result = dispatch(abi, LINUX_SYS_CHDIR, [OPEN_DIR_PATH.as_ptr() as u64, 0, 0, 0, 0, 0]).value;
    let mut readlink_buffer = [0u8; 128];
    let readlinkat_result = dispatch(
        abi,
        LINUX_SYS_READLINKAT,
        [
            AT_FDCWD as u64,
            READLINK_PATH.as_ptr() as u64,
            readlink_buffer.as_mut_ptr() as u64,
            readlink_buffer.len() as u64,
            0,
            0,
        ],
    )
    .value;
    let mut read_buffer = [0u8; 64];
    let mut read_result = -EBADF;
    let mut fstat_result = -EBADF;
    let mut dup_result = -EBADF;
    let mut dup2_result = -EBADF;
    let mut dup3_result = -EBADF;
    let mut fcntl_getfd_result = -EBADF;
    let mut fcntl_getfl_result = -EBADF;
    let mut getdents64_result = -EBADF;
    let mut fchdir_result = -EBADF;
    let mut lseek_result = -EBADF;
    let mut close_result = -EBADF;
    let mut pread64_result = -EBADF;
    let mut pwrite64_result = -EBADF;
    let mut readv_result = -EBADF;
    if openat_result >= 0 {
        let fd = openat_result as u64;
        read_result = dispatch(
            abi,
            LINUX_SYS_READ,
            [fd, read_buffer.as_mut_ptr() as u64, read_buffer.len() as u64, 0, 0, 0],
        )
        .value;
        let mut stat = LinuxStat::empty();
        fstat_result = dispatch(
            abi,
            LINUX_SYS_FSTAT,
            [fd, (&mut stat as *mut LinuxStat) as u64, 0, 0, 0, 0],
        )
        .value;
        let mut pread_buffer = [0u8; 32];
        pread64_result = dispatch(
            abi,
            LINUX_SYS_PREAD64,
            [fd, pread_buffer.as_mut_ptr() as u64, pread_buffer.len() as u64, 0, 0, 0],
        )
        .value;
        let mut readv_a = [0u8; 24];
        let mut readv_b = [0u8; 24];
        let readv_iov = [
            LinuxIovec {
                iov_base: readv_a.as_mut_ptr() as u64,
                iov_len: readv_a.len() as u64,
            },
            LinuxIovec {
                iov_base: readv_b.as_mut_ptr() as u64,
                iov_len: readv_b.len() as u64,
            },
        ];
        readv_result = dispatch(
            abi,
            LINUX_SYS_READV,
            [fd, readv_iov.as_ptr() as u64, readv_iov.len() as u64, 0, 0, 0],
        )
        .value;
        pwrite64_result = dispatch(
            abi,
            LINUX_SYS_PWRITE64,
            [fd, WRITEV_SMOKE_A.as_ptr() as u64, WRITEV_SMOKE_A.len() as u64, 0, 0, 0],
        )
        .value;
        fcntl_getfd_result = dispatch(abi, LINUX_SYS_FCNTL, [fd, F_GETFD as u64, 0, 0, 0, 0]).value;
        fcntl_getfl_result = dispatch(abi, LINUX_SYS_FCNTL, [fd, F_GETFL as u64, 0, 0, 0, 0]).value;
        dup_result = dispatch(abi, LINUX_SYS_DUP, [fd, 0, 0, 0, 0, 0]).value;
        if dup_result >= 0 {
            let _ = dispatch(abi, LINUX_SYS_CLOSE, [dup_result as u64, 0, 0, 0, 0, 0]).value;
        }
        dup2_result = dispatch(abi, LINUX_SYS_DUP2, [fd, 62, 0, 0, 0, 0]).value;
        if dup2_result >= 0 {
            let _ = dispatch(abi, LINUX_SYS_CLOSE, [dup2_result as u64, 0, 0, 0, 0, 0]).value;
        }
        dup3_result = dispatch(abi, LINUX_SYS_DUP3, [fd, 63, O_CLOEXEC, 0, 0, 0]).value;
        if dup3_result >= 0 {
            let _ = dispatch(abi, LINUX_SYS_CLOSE, [dup3_result as u64, 0, 0, 0, 0, 0]).value;
        }
        lseek_result = dispatch(abi, LINUX_SYS_LSEEK, [fd, 0, SEEK_SET as u64, 0, 0, 0]).value;
        close_result = dispatch(abi, LINUX_SYS_CLOSE, [fd, 0, 0, 0, 0, 0]).value;
    }
    let open_dir_result = dispatch(
        abi,
        LINUX_SYS_OPENAT,
        [AT_FDCWD as u64, OPEN_DIR_PATH.as_ptr() as u64, 0, 0, 0, 0],
    )
    .value;
    if open_dir_result >= 0 {
        let fd = open_dir_result as u64;
        fchdir_result = dispatch(abi, LINUX_SYS_FCHDIR, [fd, 0, 0, 0, 0, 0]).value;
        let mut dir_buffer = [0u8; 256];
        getdents64_result = dispatch(
            abi,
            LINUX_SYS_GETDENTS64,
            [fd, dir_buffer.as_mut_ptr() as u64, dir_buffer.len() as u64, 0, 0, 0],
        )
        .value;
        let _ = dispatch(abi, LINUX_SYS_CLOSE, [fd, 0, 0, 0, 0, 0]).value;
    }
    let _ = dispatch(abi, LINUX_SYS_CHDIR, [ROOT_PATH.as_ptr() as u64, 0, 0, 0, 0, 0]).value;

    let getpid_result = dispatch(abi, LINUX_SYS_GETPID, [0; 6]).value;
    let getppid_result = dispatch(abi, LINUX_SYS_GETPPID, [0; 6]).value;
    let gettid_result = dispatch(abi, LINUX_SYS_GETTID, [0; 6]).value;
    let umask_result = dispatch(abi, LINUX_SYS_UMASK, [0o077, 0, 0, 0, 0, 0]).value;
    let umask_restore_value = if umask_result >= 0 {
        umask_result as u64
    } else {
        DEFAULT_PROCESS_UMASK as u64
    };
    let umask_restore_result = dispatch(abi, LINUX_SYS_UMASK, [umask_restore_value, 0, 0, 0, 0, 0]).value;
    let getuid_result = dispatch(abi, LINUX_SYS_GETUID, [0; 6]).value;
    let getgid_result = dispatch(abi, LINUX_SYS_GETGID, [0; 6]).value;
    let geteuid_result = dispatch(abi, LINUX_SYS_GETEUID, [0; 6]).value;
    let getegid_result = dispatch(abi, LINUX_SYS_GETEGID, [0; 6]).value;
    let mut clear_tid_slot = 0i32;
    let set_tid_address_result = dispatch(
        abi,
        LINUX_SYS_SET_TID_ADDRESS,
        [(&mut clear_tid_slot as *mut i32) as u64, 0, 0, 0, 0, 0],
    )
    .value;
    let clear_tid_snapshot = i64::from(clear_tid_slot);
    let sched_yield_result = dispatch(abi, LINUX_SYS_SCHED_YIELD, [0; 6]).value;

    let mut timespec = LinuxTimespec { tv_sec: 0, tv_nsec: 0 };
    let clock_gettime_result = dispatch(
        abi,
        LINUX_SYS_CLOCK_GETTIME,
        [
            LINUX_CLOCK_MONOTONIC as u64,
            (&mut timespec as *mut LinuxTimespec) as u64,
            0,
            0,
            0,
            0,
        ],
    )
    .value;

    let mut utsname = LinuxUtsName::new();
    let uname_result = dispatch(
        abi,
        LINUX_SYS_UNAME,
        [(&mut utsname as *mut LinuxUtsName) as u64, 0, 0, 0, 0, 0],
    )
    .value;

    let exit_group = dispatch(abi, LINUX_SYS_EXIT_GROUP, [17, 0, 0, 0, 0, 0]);
    let (exit_group_captured, exit_group_status) = exit_status(exit_group);

    let mut machine_bytes = [0u8; 16];
    let machine_len = copy_c_field_prefix(&mut machine_bytes, &utsname.machine);

    LinuxBootstrapProbe {
        write_result,
        openat_result,
        mmap_result,
        mprotect_result,
        munmap_result,
        brk_result,
        brk_set_result,
        brk_restore_result,
        nanosleep_result,
        gettimeofday_result,
        gettimeofday_seconds: timeval.tv_sec,
        gettimeofday_microseconds: timeval.tv_usec,
        getrandom_result,
        getrandom_sample,
        rt_sigaction_result,
        rt_sigprocmask_result,
        rt_sigmask_snapshot,
        rt_sigold_handler,
        wait4_result,
        setpgid_result,
        getpgid_result,
        setsid_result,
        getsid_result,
        getrlimit_result,
        setrlimit_result,
        prlimit64_result,
        prctl_set_name_result,
        prctl_get_name_result,
        prctl_set_dumpable_result,
        prctl_get_dumpable_result,
        set_robust_list_result,
        get_robust_list_result,
        rseq_register_result,
        rseq_unregister_result,
        pread64_result,
        pwrite64_result,
        readv_result,
        writev_result,
        ioctl_result,
        access_result,
        newfstatat_result,
        faccessat_result,
        faccessat2_result,
        readlinkat_result,
        dup_result,
        dup2_result,
        dup3_result,
        fcntl_getfd_result,
        fcntl_getfl_result,
        getcwd_result,
        chdir_result,
        fchdir_result,
        read_result,
        fstat_result,
        getdents64_result,
        lseek_result,
        close_result,
        getpid_result,
        getppid_result,
        gettid_result,
        umask_result,
        umask_restore_result,
        getuid_result,
        getgid_result,
        geteuid_result,
        getegid_result,
        set_tid_address_result,
        clear_tid_snapshot,
        sched_yield_result,
        clock_gettime_result,
        clock_seconds: timespec.tv_sec,
        clock_nanoseconds: timespec.tv_nsec,
        uname_result,
        exit_group_captured,
        exit_group_status,
        machine_bytes,
        machine_len,
    }
}

pub fn run_ghost_bootstrap_probe() -> GhostBootstrapProbe {
    static WRITE_SMOKE: &[u8] = b"HXNU: ghost syscall write() compatibility smoke\n";
    static WRITEV_SMOKE_A: &[u8] = b"HXNU: ghost writev ";
    static WRITEV_SMOKE_B: &[u8] = b"compatibility smoke\n";
    static OPEN_PATH: &[u8] = b"/proc/version\0";
    static OPEN_DIR_PATH: &[u8] = b"/proc\0";
    static ROOT_PATH: &[u8] = b"/\0";
    static READLINK_PATH: &[u8] = b"/proc/self/exe\0";
    let abi = SyscallAbi::GhostBootstrap;

    let write_result = dispatch(
        abi,
        GHOST_SYS_WRITE,
        [
            STDERR_FD,
            WRITE_SMOKE.as_ptr() as u64,
            WRITE_SMOKE.len() as u64,
            0,
            0,
            0,
        ],
    )
    .value;
    let open_result = dispatch(abi, GHOST_SYS_OPEN, [OPEN_PATH.as_ptr() as u64, 0, 0, 0, 0, 0]).value;
    let mmap_result = dispatch(
        abi,
        GHOST_SYS_MMAP,
        [
            0,
            MMAP_PAGE_SIZE as u64,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            u64::MAX,
            0,
        ],
    )
    .value;
    let mut mprotect_result = -EINVAL;
    let mut munmap_result = -EINVAL;
    if mmap_result >= 0 {
        let address = mmap_result as u64;
        mprotect_result = dispatch(
            abi,
            GHOST_SYS_MPROTECT,
            [address, MMAP_PAGE_SIZE as u64, PROT_READ, 0, 0, 0],
        )
        .value;
        munmap_result = dispatch(abi, GHOST_SYS_MUNMAP, [address, MMAP_PAGE_SIZE as u64, 0, 0, 0, 0]).value;
    }
    let brk_result = dispatch(abi, GHOST_SYS_BRK, [0, 0, 0, 0, 0, 0]).value;
    let brk_set_target = if brk_result >= 0 {
        (brk_result as u64).saturating_add(MMAP_PAGE_SIZE as u64)
    } else {
        (DEFAULT_PROCESS_BRK as u64).saturating_add(MMAP_PAGE_SIZE as u64)
    };
    let brk_set_result = dispatch(abi, GHOST_SYS_BRK, [brk_set_target, 0, 0, 0, 0, 0]).value;
    let brk_restore_target = if brk_result >= 0 {
        brk_result as u64
    } else {
        DEFAULT_PROCESS_BRK as u64
    };
    let brk_restore_result = dispatch(abi, GHOST_SYS_BRK, [brk_restore_target, 0, 0, 0, 0, 0]).value;
    let nanosleep_request = LinuxTimespec {
        tv_sec: 0,
        tv_nsec: 500_000,
    };
    let nanosleep_result = dispatch(
        abi,
        GHOST_SYS_NANOSLEEP,
        [(&nanosleep_request as *const LinuxTimespec) as u64, 0, 0, 0, 0, 0],
    )
    .value;
    let mut timeval = LinuxTimeval { tv_sec: 0, tv_usec: 0 };
    let gettimeofday_result = dispatch(
        abi,
        GHOST_SYS_GETTIMEOFDAY,
        [(&mut timeval as *mut LinuxTimeval) as u64, 0, 0, 0, 0, 0],
    )
    .value;
    let mut random_buffer = [0u8; 16];
    let getrandom_result = dispatch(
        abi,
        GHOST_SYS_GETRANDOM,
        [
            random_buffer.as_mut_ptr() as u64,
            random_buffer.len() as u64,
            GRND_NONBLOCK,
            0,
            0,
            0,
        ],
    )
    .value;
    let getrandom_sample = sample_random_u64(&random_buffer);
    let signal_set = 1u64 << 1;
    let mut previous_signal_mask = 0u64;
    let rt_sigprocmask_result = dispatch(
        abi,
        GHOST_SYS_RT_SIGPROCMASK,
        [
            SIG_BLOCK as u64,
            (&signal_set as *const u64) as u64,
            (&mut previous_signal_mask as *mut u64) as u64,
            RT_SIGSET_SIZE as u64,
            0,
            0,
        ],
    )
    .value;
    let action = LinuxKernelSigAction {
        handler: 0x10,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    let mut old_action = LinuxKernelSigAction::empty();
    let rt_sigaction_result = dispatch(
        abi,
        GHOST_SYS_RT_SIGACTION,
        [
            10,
            (&action as *const LinuxKernelSigAction) as u64,
            (&mut old_action as *mut LinuxKernelSigAction) as u64,
            RT_SIGSET_SIZE as u64,
            0,
            0,
        ],
    )
    .value;
    let rt_sigmask_snapshot = previous_signal_mask;
    let rt_sigold_handler = old_action.handler;
    let wait4_result = dispatch(abi, GHOST_SYS_WAIT4, [u64::MAX, 0, WNOHANG as u64, 0, 0, 0]).value;
    let setpgid_result = dispatch(abi, GHOST_SYS_SETPGID, [0, 0, 0, 0, 0, 0]).value;
    let getpgid_result = dispatch(abi, GHOST_SYS_GETPGID, [0, 0, 0, 0, 0, 0]).value;
    let setsid_result = dispatch(abi, GHOST_SYS_SETSID, [0, 0, 0, 0, 0, 0]).value;
    let getsid_result = dispatch(abi, GHOST_SYS_GETSID, [0, 0, 0, 0, 0, 0]).value;
    let mut nofile_limit = LinuxRlimit64 {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let getrlimit_result = dispatch(
        abi,
        GHOST_SYS_GETRLIMIT,
        [
            RLIMIT_NOFILE as u64,
            (&mut nofile_limit as *mut LinuxRlimit64) as u64,
            0,
            0,
            0,
            0,
        ],
    )
    .value;
    let setrlimit_result = dispatch(
        abi,
        GHOST_SYS_SETRLIMIT,
        [
            RLIMIT_NOFILE as u64,
            (&nofile_limit as *const LinuxRlimit64) as u64,
            0,
            0,
            0,
            0,
        ],
    )
    .value;
    let mut prlimit_old = LinuxRlimit64 {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let prlimit64_result = dispatch(
        abi,
        GHOST_SYS_PRLIMIT64,
        [
            0,
            RLIMIT_NOFILE as u64,
            0,
            (&mut prlimit_old as *mut LinuxRlimit64) as u64,
            0,
            0,
        ],
    )
    .value;
    let prctl_name = b"ghost-bootstrap\0";
    let prctl_set_name_result = dispatch(
        abi,
        GHOST_SYS_PRCTL,
        [PR_SET_NAME as u64, prctl_name.as_ptr() as u64, 0, 0, 0, 0],
    )
    .value;
    let mut prctl_name_readback = [0u8; TASK_COMM_LEN];
    let prctl_get_name_result = dispatch(
        abi,
        GHOST_SYS_PRCTL,
        [PR_GET_NAME as u64, prctl_name_readback.as_mut_ptr() as u64, 0, 0, 0, 0],
    )
    .value;
    let prctl_set_dumpable_result = dispatch(
        abi,
        GHOST_SYS_PRCTL,
        [PR_SET_DUMPABLE as u64, 1, 0, 0, 0, 0],
    )
    .value;
    let prctl_get_dumpable_result = dispatch(abi, GHOST_SYS_PRCTL, [PR_GET_DUMPABLE as u64, 0, 0, 0, 0, 0]).value;
    let robust_head = LinuxRobustListHead {
        list_next: 0,
        futex_offset: 0,
        list_op_pending: 0,
    };
    let set_robust_list_result = dispatch(
        abi,
        GHOST_SYS_SET_ROBUST_LIST,
        [
            (&robust_head as *const LinuxRobustListHead) as u64,
            size_of::<LinuxRobustListHead>() as u64,
            0,
            0,
            0,
            0,
        ],
    )
    .value;
    let mut robust_head_readback = 0u64;
    let mut robust_len_readback = 0usize;
    let get_robust_list_result = dispatch(
        abi,
        GHOST_SYS_GET_ROBUST_LIST,
        [
            0,
            (&mut robust_head_readback as *mut u64) as u64,
            (&mut robust_len_readback as *mut usize) as u64,
            0,
            0,
            0,
        ],
    )
    .value;
    let rseq_area = LinuxRseqArea::empty();
    let rseq_register_result = dispatch(
        abi,
        GHOST_SYS_RSEQ,
        [
            (&rseq_area as *const LinuxRseqArea) as u64,
            size_of::<LinuxRseqArea>() as u64,
            0,
            RSEQ_SIGNATURE as u64,
            0,
            0,
        ],
    )
    .value;
    let rseq_unregister_result = dispatch(
        abi,
        GHOST_SYS_RSEQ,
        [
            (&rseq_area as *const LinuxRseqArea) as u64,
            size_of::<LinuxRseqArea>() as u64,
            RSEQ_FLAG_UNREGISTER as u64,
            RSEQ_SIGNATURE as u64,
            0,
            0,
        ],
    )
    .value;
    let writev_iov = [
        LinuxIovec {
            iov_base: WRITEV_SMOKE_A.as_ptr() as u64,
            iov_len: WRITEV_SMOKE_A.len() as u64,
        },
        LinuxIovec {
            iov_base: WRITEV_SMOKE_B.as_ptr() as u64,
            iov_len: WRITEV_SMOKE_B.len() as u64,
        },
    ];
    let writev_result = dispatch(
        abi,
        GHOST_SYS_WRITEV,
        [STDERR_FD, writev_iov.as_ptr() as u64, writev_iov.len() as u64, 0, 0, 0],
    )
    .value;
    let mut winsize = LinuxWinsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let ioctl_result = dispatch(
        abi,
        GHOST_SYS_IOCTL,
        [STDERR_FD, LINUX_TIOCGWINSZ, (&mut winsize as *mut LinuxWinsize) as u64, 0, 0, 0],
    )
    .value;
    let access_result = dispatch(abi, GHOST_SYS_ACCESS, [OPEN_PATH.as_ptr() as u64, R_OK, 0, 0, 0, 0]).value;
    let mut stat = LinuxStat::empty();
    let stat_result = dispatch(
        abi,
        GHOST_SYS_STAT,
        [OPEN_PATH.as_ptr() as u64, (&mut stat as *mut LinuxStat) as u64, 0, 0, 0, 0],
    )
    .value;
    let mut readlink_buffer = [0u8; 128];
    let readlink_result = dispatch(
        abi,
        GHOST_SYS_READLINK,
        [
            READLINK_PATH.as_ptr() as u64,
            readlink_buffer.as_mut_ptr() as u64,
            readlink_buffer.len() as u64,
            0,
            0,
            0,
        ],
    )
    .value;
    let mut cwd_buffer = [0u8; 128];
    let getcwd_result = dispatch(
        abi,
        GHOST_SYS_GETCWD,
        [cwd_buffer.as_mut_ptr() as u64, cwd_buffer.len() as u64, 0, 0, 0, 0],
    )
    .value;
    let chdir_result = dispatch(abi, GHOST_SYS_CHDIR, [OPEN_DIR_PATH.as_ptr() as u64, 0, 0, 0, 0, 0]).value;

    let mut read_buffer = [0u8; 64];
    let mut read_result = -EBADF;
    let mut fstat_result = -EBADF;
    let mut dup_result = -EBADF;
    let mut dup2_result = -EBADF;
    let mut dup3_result = -EBADF;
    let mut fcntl_getfd_result = -EBADF;
    let mut fcntl_getfl_result = -EBADF;
    let mut getdents_result = -EBADF;
    let mut fchdir_result = -EBADF;
    let mut seek_result = -EBADF;
    let mut close_result = -EBADF;
    let mut pread64_result = -EBADF;
    let mut pwrite64_result = -EBADF;
    let mut readv_result = -EBADF;
    if open_result >= 0 {
        let fd = open_result as u64;
        read_result = dispatch(
            abi,
            GHOST_SYS_READ,
            [fd, read_buffer.as_mut_ptr() as u64, read_buffer.len() as u64, 0, 0, 0],
        )
        .value;
        let mut stat = LinuxStat::empty();
        fstat_result = dispatch(
            abi,
            GHOST_SYS_FSTAT,
            [fd, (&mut stat as *mut LinuxStat) as u64, 0, 0, 0, 0],
        )
        .value;
        let mut pread_buffer = [0u8; 32];
        pread64_result = dispatch(
            abi,
            GHOST_SYS_PREAD64,
            [fd, pread_buffer.as_mut_ptr() as u64, pread_buffer.len() as u64, 0, 0, 0],
        )
        .value;
        let mut readv_a = [0u8; 24];
        let mut readv_b = [0u8; 24];
        let readv_iov = [
            LinuxIovec {
                iov_base: readv_a.as_mut_ptr() as u64,
                iov_len: readv_a.len() as u64,
            },
            LinuxIovec {
                iov_base: readv_b.as_mut_ptr() as u64,
                iov_len: readv_b.len() as u64,
            },
        ];
        readv_result = dispatch(
            abi,
            GHOST_SYS_READV,
            [fd, readv_iov.as_ptr() as u64, readv_iov.len() as u64, 0, 0, 0],
        )
        .value;
        pwrite64_result = dispatch(
            abi,
            GHOST_SYS_PWRITE64,
            [fd, WRITEV_SMOKE_A.as_ptr() as u64, WRITEV_SMOKE_A.len() as u64, 0, 0, 0],
        )
        .value;
        fcntl_getfd_result = dispatch(abi, GHOST_SYS_FCNTL, [fd, F_GETFD as u64, 0, 0, 0, 0]).value;
        fcntl_getfl_result = dispatch(abi, GHOST_SYS_FCNTL, [fd, F_GETFL as u64, 0, 0, 0, 0]).value;
        dup_result = dispatch(abi, GHOST_SYS_DUP, [fd, 0, 0, 0, 0, 0]).value;
        if dup_result >= 0 {
            let _ = dispatch(abi, GHOST_SYS_CLOSE, [dup_result as u64, 0, 0, 0, 0, 0]).value;
        }
        dup2_result = dispatch(abi, GHOST_SYS_DUP2, [fd, 62, 0, 0, 0, 0]).value;
        if dup2_result >= 0 {
            let _ = dispatch(abi, GHOST_SYS_CLOSE, [dup2_result as u64, 0, 0, 0, 0, 0]).value;
        }
        dup3_result = dispatch(abi, GHOST_SYS_DUP3, [fd, 63, O_CLOEXEC, 0, 0, 0]).value;
        if dup3_result >= 0 {
            let _ = dispatch(abi, GHOST_SYS_CLOSE, [dup3_result as u64, 0, 0, 0, 0, 0]).value;
        }
        seek_result = dispatch(abi, GHOST_SYS_SEEK, [fd, 0, SEEK_SET as u64, 0, 0, 0]).value;
        close_result = dispatch(abi, GHOST_SYS_CLOSE, [fd, 0, 0, 0, 0, 0]).value;
    }
    let open_dir_result = dispatch(abi, GHOST_SYS_OPEN, [OPEN_DIR_PATH.as_ptr() as u64, 0, 0, 0, 0, 0]).value;
    if open_dir_result >= 0 {
        let fd = open_dir_result as u64;
        fchdir_result = dispatch(abi, GHOST_SYS_FCHDIR, [fd, 0, 0, 0, 0, 0]).value;
        let mut dir_buffer = [0u8; 256];
        getdents_result = dispatch(
            abi,
            GHOST_SYS_GETDENTS,
            [fd, dir_buffer.as_mut_ptr() as u64, dir_buffer.len() as u64, 0, 0, 0],
        )
        .value;
        let _ = dispatch(abi, GHOST_SYS_CLOSE, [fd, 0, 0, 0, 0, 0]).value;
    }
    let _ = dispatch(abi, GHOST_SYS_CHDIR, [ROOT_PATH.as_ptr() as u64, 0, 0, 0, 0, 0]).value;

    let getpid_result = dispatch(abi, GHOST_SYS_GETPID, [0; 6]).value;
    let getppid_result = dispatch(abi, GHOST_SYS_GETPPID, [0; 6]).value;
    let gettid_result = dispatch(abi, GHOST_SYS_GETTID, [0; 6]).value;
    let umask_result = dispatch(abi, GHOST_SYS_UMASK, [0o077, 0, 0, 0, 0, 0]).value;
    let umask_restore_value = if umask_result >= 0 {
        umask_result as u64
    } else {
        DEFAULT_PROCESS_UMASK as u64
    };
    let umask_restore_result = dispatch(abi, GHOST_SYS_UMASK, [umask_restore_value, 0, 0, 0, 0, 0]).value;
    let getuid_result = dispatch(abi, GHOST_SYS_GETUID, [0; 6]).value;
    let getgid_result = dispatch(abi, GHOST_SYS_GETGID, [0; 6]).value;
    let geteuid_result = dispatch(abi, GHOST_SYS_GETEUID, [0; 6]).value;
    let getegid_result = dispatch(abi, GHOST_SYS_GETEGID, [0; 6]).value;
    let mut clear_tid_slot = 0i32;
    let set_tid_address_result = dispatch(
        abi,
        GHOST_SYS_SET_TID_ADDRESS,
        [(&mut clear_tid_slot as *mut i32) as u64, 0, 0, 0, 0, 0],
    )
    .value;
    let clear_tid_snapshot = i64::from(clear_tid_slot);
    let yield_result = dispatch(abi, GHOST_SYS_YIELD, [0; 6]).value;
    let uptime_result = dispatch(abi, GHOST_SYS_UPTIME_NSEC, [0; 6]).value;

    let mut utsname = LinuxUtsName::new();
    let uname_result = dispatch(
        abi,
        GHOST_SYS_UNAME,
        [(&mut utsname as *mut LinuxUtsName) as u64, 0, 0, 0, 0, 0],
    )
    .value;

    let exit_group = dispatch(abi, GHOST_SYS_EXIT_GROUP, [19, 0, 0, 0, 0, 0]);
    let (exit_group_captured, exit_group_status) = exit_status(exit_group);

    let mut machine_bytes = [0u8; 16];
    let machine_len = copy_c_field_prefix(&mut machine_bytes, &utsname.machine);

    GhostBootstrapProbe {
        write_result,
        open_result,
        mmap_result,
        mprotect_result,
        munmap_result,
        brk_result,
        brk_set_result,
        brk_restore_result,
        nanosleep_result,
        gettimeofday_result,
        gettimeofday_seconds: timeval.tv_sec,
        gettimeofday_microseconds: timeval.tv_usec,
        getrandom_result,
        getrandom_sample,
        rt_sigaction_result,
        rt_sigprocmask_result,
        rt_sigmask_snapshot,
        rt_sigold_handler,
        wait4_result,
        setpgid_result,
        getpgid_result,
        setsid_result,
        getsid_result,
        getrlimit_result,
        setrlimit_result,
        prlimit64_result,
        prctl_set_name_result,
        prctl_get_name_result,
        prctl_set_dumpable_result,
        prctl_get_dumpable_result,
        set_robust_list_result,
        get_robust_list_result,
        rseq_register_result,
        rseq_unregister_result,
        pread64_result,
        pwrite64_result,
        readv_result,
        writev_result,
        ioctl_result,
        access_result,
        stat_result,
        readlink_result,
        dup_result,
        dup2_result,
        dup3_result,
        fcntl_getfd_result,
        fcntl_getfl_result,
        getcwd_result,
        chdir_result,
        fchdir_result,
        read_result,
        fstat_result,
        getdents_result,
        seek_result,
        close_result,
        getpid_result,
        getppid_result,
        gettid_result,
        umask_result,
        umask_restore_result,
        getuid_result,
        getgid_result,
        geteuid_result,
        getegid_result,
        set_tid_address_result,
        clear_tid_snapshot,
        yield_result,
        uptime_result,
        uname_result,
        exit_group_captured,
        exit_group_status,
        machine_bytes,
        machine_len,
    }
}

pub fn run_hxnu_bootstrap_probe() -> HxnuBootstrapProbe {
    static WRITE_SMOKE: &[u8] = b"HXNU: native syscall log_write() bootstrap smoke\n";
    static WRITEV_SMOKE_A: &[u8] = b"HXNU: hxnu writev ";
    static WRITEV_SMOKE_B: &[u8] = b"compatibility smoke\n";
    static OPEN_PATH: &[u8] = b"/proc/version\0";
    static OPEN_DIR_PATH: &[u8] = b"/proc\0";
    static ROOT_PATH: &[u8] = b"/\0";
    static READLINK_PATH: &[u8] = b"/proc/self/exe\0";
    let abi = SyscallAbi::HxnuNativeBootstrap;

    let write_result = dispatch(
        abi,
        HXNU_SYS_LOG_WRITE,
        [WRITE_SMOKE.as_ptr() as u64, WRITE_SMOKE.len() as u64, 0, 0, 0, 0],
    )
    .value;
    let open_result = dispatch(abi, HXNU_SYS_OPEN, [OPEN_PATH.as_ptr() as u64, 0, 0, 0, 0, 0]).value;
    let mmap_result = dispatch(
        abi,
        HXNU_SYS_MMAP,
        [
            0,
            MMAP_PAGE_SIZE as u64,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            u64::MAX,
            0,
        ],
    )
    .value;
    let mut mprotect_result = -EINVAL;
    let mut munmap_result = -EINVAL;
    if mmap_result >= 0 {
        let address = mmap_result as u64;
        mprotect_result = dispatch(
            abi,
            HXNU_SYS_MPROTECT,
            [address, MMAP_PAGE_SIZE as u64, PROT_READ, 0, 0, 0],
        )
        .value;
        munmap_result = dispatch(abi, HXNU_SYS_MUNMAP, [address, MMAP_PAGE_SIZE as u64, 0, 0, 0, 0]).value;
    }
    let brk_result = dispatch(abi, HXNU_SYS_BRK, [0, 0, 0, 0, 0, 0]).value;
    let brk_set_target = if brk_result >= 0 {
        (brk_result as u64).saturating_add(MMAP_PAGE_SIZE as u64)
    } else {
        (DEFAULT_PROCESS_BRK as u64).saturating_add(MMAP_PAGE_SIZE as u64)
    };
    let brk_set_result = dispatch(abi, HXNU_SYS_BRK, [brk_set_target, 0, 0, 0, 0, 0]).value;
    let brk_restore_target = if brk_result >= 0 {
        brk_result as u64
    } else {
        DEFAULT_PROCESS_BRK as u64
    };
    let brk_restore_result = dispatch(abi, HXNU_SYS_BRK, [brk_restore_target, 0, 0, 0, 0, 0]).value;
    let nanosleep_request = LinuxTimespec {
        tv_sec: 0,
        tv_nsec: 500_000,
    };
    let nanosleep_result = dispatch(
        abi,
        HXNU_SYS_NANOSLEEP,
        [(&nanosleep_request as *const LinuxTimespec) as u64, 0, 0, 0, 0, 0],
    )
    .value;
    let mut timeval = LinuxTimeval { tv_sec: 0, tv_usec: 0 };
    let gettimeofday_result = dispatch(
        abi,
        HXNU_SYS_GETTIMEOFDAY,
        [(&mut timeval as *mut LinuxTimeval) as u64, 0, 0, 0, 0, 0],
    )
    .value;
    let mut random_buffer = [0u8; 16];
    let getrandom_result = dispatch(
        abi,
        HXNU_SYS_GETRANDOM,
        [
            random_buffer.as_mut_ptr() as u64,
            random_buffer.len() as u64,
            GRND_NONBLOCK,
            0,
            0,
            0,
        ],
    )
    .value;
    let getrandom_sample = sample_random_u64(&random_buffer);
    let signal_set = 1u64 << 1;
    let mut previous_signal_mask = 0u64;
    let rt_sigprocmask_result = dispatch(
        abi,
        HXNU_SYS_RT_SIGPROCMASK,
        [
            SIG_BLOCK as u64,
            (&signal_set as *const u64) as u64,
            (&mut previous_signal_mask as *mut u64) as u64,
            RT_SIGSET_SIZE as u64,
            0,
            0,
        ],
    )
    .value;
    let action = LinuxKernelSigAction {
        handler: 0x10,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    let mut old_action = LinuxKernelSigAction::empty();
    let rt_sigaction_result = dispatch(
        abi,
        HXNU_SYS_RT_SIGACTION,
        [
            10,
            (&action as *const LinuxKernelSigAction) as u64,
            (&mut old_action as *mut LinuxKernelSigAction) as u64,
            RT_SIGSET_SIZE as u64,
            0,
            0,
        ],
    )
    .value;
    let rt_sigmask_snapshot = previous_signal_mask;
    let rt_sigold_handler = old_action.handler;
    let wait4_result = dispatch(abi, HXNU_SYS_WAIT4, [u64::MAX, 0, WNOHANG as u64, 0, 0, 0]).value;
    let setpgid_result = dispatch(abi, HXNU_SYS_SETPGID, [0, 0, 0, 0, 0, 0]).value;
    let getpgid_result = dispatch(abi, HXNU_SYS_GETPGID, [0, 0, 0, 0, 0, 0]).value;
    let setsid_result = dispatch(abi, HXNU_SYS_SETSID, [0, 0, 0, 0, 0, 0]).value;
    let getsid_result = dispatch(abi, HXNU_SYS_GETSID, [0, 0, 0, 0, 0, 0]).value;
    let mut nofile_limit = LinuxRlimit64 {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let getrlimit_result = dispatch(
        abi,
        HXNU_SYS_GETRLIMIT,
        [
            RLIMIT_NOFILE as u64,
            (&mut nofile_limit as *mut LinuxRlimit64) as u64,
            0,
            0,
            0,
            0,
        ],
    )
    .value;
    let setrlimit_result = dispatch(
        abi,
        HXNU_SYS_SETRLIMIT,
        [
            RLIMIT_NOFILE as u64,
            (&nofile_limit as *const LinuxRlimit64) as u64,
            0,
            0,
            0,
            0,
        ],
    )
    .value;
    let mut prlimit_old = LinuxRlimit64 {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let prlimit64_result = dispatch(
        abi,
        HXNU_SYS_PRLIMIT64,
        [
            0,
            RLIMIT_NOFILE as u64,
            0,
            (&mut prlimit_old as *mut LinuxRlimit64) as u64,
            0,
            0,
        ],
    )
    .value;
    let prctl_name = b"hxnu-bootstrap\0";
    let prctl_set_name_result = dispatch(
        abi,
        HXNU_SYS_PRCTL,
        [PR_SET_NAME as u64, prctl_name.as_ptr() as u64, 0, 0, 0, 0],
    )
    .value;
    let mut prctl_name_readback = [0u8; TASK_COMM_LEN];
    let prctl_get_name_result = dispatch(
        abi,
        HXNU_SYS_PRCTL,
        [PR_GET_NAME as u64, prctl_name_readback.as_mut_ptr() as u64, 0, 0, 0, 0],
    )
    .value;
    let prctl_set_dumpable_result = dispatch(
        abi,
        HXNU_SYS_PRCTL,
        [PR_SET_DUMPABLE as u64, 1, 0, 0, 0, 0],
    )
    .value;
    let prctl_get_dumpable_result = dispatch(abi, HXNU_SYS_PRCTL, [PR_GET_DUMPABLE as u64, 0, 0, 0, 0, 0]).value;
    let robust_head = LinuxRobustListHead {
        list_next: 0,
        futex_offset: 0,
        list_op_pending: 0,
    };
    let set_robust_list_result = dispatch(
        abi,
        HXNU_SYS_SET_ROBUST_LIST,
        [
            (&robust_head as *const LinuxRobustListHead) as u64,
            size_of::<LinuxRobustListHead>() as u64,
            0,
            0,
            0,
            0,
        ],
    )
    .value;
    let mut robust_head_readback = 0u64;
    let mut robust_len_readback = 0usize;
    let get_robust_list_result = dispatch(
        abi,
        HXNU_SYS_GET_ROBUST_LIST,
        [
            0,
            (&mut robust_head_readback as *mut u64) as u64,
            (&mut robust_len_readback as *mut usize) as u64,
            0,
            0,
            0,
        ],
    )
    .value;
    let rseq_area = LinuxRseqArea::empty();
    let rseq_register_result = dispatch(
        abi,
        HXNU_SYS_RSEQ,
        [
            (&rseq_area as *const LinuxRseqArea) as u64,
            size_of::<LinuxRseqArea>() as u64,
            0,
            RSEQ_SIGNATURE as u64,
            0,
            0,
        ],
    )
    .value;
    let rseq_unregister_result = dispatch(
        abi,
        HXNU_SYS_RSEQ,
        [
            (&rseq_area as *const LinuxRseqArea) as u64,
            size_of::<LinuxRseqArea>() as u64,
            RSEQ_FLAG_UNREGISTER as u64,
            RSEQ_SIGNATURE as u64,
            0,
            0,
        ],
    )
    .value;
    let writev_iov = [
        LinuxIovec {
            iov_base: WRITEV_SMOKE_A.as_ptr() as u64,
            iov_len: WRITEV_SMOKE_A.len() as u64,
        },
        LinuxIovec {
            iov_base: WRITEV_SMOKE_B.as_ptr() as u64,
            iov_len: WRITEV_SMOKE_B.len() as u64,
        },
    ];
    let writev_result = dispatch(
        abi,
        HXNU_SYS_WRITEV,
        [STDOUT_FD, writev_iov.as_ptr() as u64, writev_iov.len() as u64, 0, 0, 0],
    )
    .value;
    let mut winsize = LinuxWinsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let ioctl_result = dispatch(
        abi,
        HXNU_SYS_IOCTL,
        [STDOUT_FD, LINUX_TIOCGWINSZ, (&mut winsize as *mut LinuxWinsize) as u64, 0, 0, 0],
    )
    .value;
    let access_result = dispatch(abi, HXNU_SYS_ACCESS, [OPEN_PATH.as_ptr() as u64, R_OK, 0, 0, 0, 0]).value;
    let mut stat = LinuxStat::empty();
    let stat_result = dispatch(
        abi,
        HXNU_SYS_STAT,
        [OPEN_PATH.as_ptr() as u64, (&mut stat as *mut LinuxStat) as u64, 0, 0, 0, 0],
    )
    .value;
    let mut readlink_buffer = [0u8; 128];
    let readlink_result = dispatch(
        abi,
        HXNU_SYS_READLINK,
        [
            READLINK_PATH.as_ptr() as u64,
            readlink_buffer.as_mut_ptr() as u64,
            readlink_buffer.len() as u64,
            0,
            0,
            0,
        ],
    )
    .value;
    let mut cwd_buffer = [0u8; 128];
    let getcwd_result = dispatch(
        abi,
        HXNU_SYS_GETCWD,
        [cwd_buffer.as_mut_ptr() as u64, cwd_buffer.len() as u64, 0, 0, 0, 0],
    )
    .value;
    let chdir_result = dispatch(abi, HXNU_SYS_CHDIR, [OPEN_DIR_PATH.as_ptr() as u64, 0, 0, 0, 0, 0]).value;

    let mut read_buffer = [0u8; 64];
    let mut read_result = -EBADF;
    let mut fstat_result = -EBADF;
    let mut dup_result = -EBADF;
    let mut dup2_result = -EBADF;
    let mut dup3_result = -EBADF;
    let mut fcntl_getfd_result = -EBADF;
    let mut fcntl_getfl_result = -EBADF;
    let mut getdents_result = -EBADF;
    let mut fchdir_result = -EBADF;
    let mut seek_result = -EBADF;
    let mut close_result = -EBADF;
    let mut pread64_result = -EBADF;
    let mut pwrite64_result = -EBADF;
    let mut readv_result = -EBADF;
    if open_result >= 0 {
        let fd = open_result as u64;
        read_result = dispatch(
            abi,
            HXNU_SYS_READ,
            [fd, read_buffer.as_mut_ptr() as u64, read_buffer.len() as u64, 0, 0, 0],
        )
        .value;
        let mut stat = LinuxStat::empty();
        fstat_result = dispatch(
            abi,
            HXNU_SYS_FSTAT,
            [fd, (&mut stat as *mut LinuxStat) as u64, 0, 0, 0, 0],
        )
        .value;
        let mut pread_buffer = [0u8; 32];
        pread64_result = dispatch(
            abi,
            HXNU_SYS_PREAD64,
            [fd, pread_buffer.as_mut_ptr() as u64, pread_buffer.len() as u64, 0, 0, 0],
        )
        .value;
        let mut readv_a = [0u8; 24];
        let mut readv_b = [0u8; 24];
        let readv_iov = [
            LinuxIovec {
                iov_base: readv_a.as_mut_ptr() as u64,
                iov_len: readv_a.len() as u64,
            },
            LinuxIovec {
                iov_base: readv_b.as_mut_ptr() as u64,
                iov_len: readv_b.len() as u64,
            },
        ];
        readv_result = dispatch(
            abi,
            HXNU_SYS_READV,
            [fd, readv_iov.as_ptr() as u64, readv_iov.len() as u64, 0, 0, 0],
        )
        .value;
        pwrite64_result = dispatch(
            abi,
            HXNU_SYS_PWRITE64,
            [fd, WRITEV_SMOKE_A.as_ptr() as u64, WRITEV_SMOKE_A.len() as u64, 0, 0, 0],
        )
        .value;
        fcntl_getfd_result = dispatch(abi, HXNU_SYS_FCNTL, [fd, F_GETFD as u64, 0, 0, 0, 0]).value;
        fcntl_getfl_result = dispatch(abi, HXNU_SYS_FCNTL, [fd, F_GETFL as u64, 0, 0, 0, 0]).value;
        dup_result = dispatch(abi, HXNU_SYS_DUP, [fd, 0, 0, 0, 0, 0]).value;
        if dup_result >= 0 {
            let _ = dispatch(abi, HXNU_SYS_CLOSE, [dup_result as u64, 0, 0, 0, 0, 0]).value;
        }
        dup2_result = dispatch(abi, HXNU_SYS_DUP2, [fd, 62, 0, 0, 0, 0]).value;
        if dup2_result >= 0 {
            let _ = dispatch(abi, HXNU_SYS_CLOSE, [dup2_result as u64, 0, 0, 0, 0, 0]).value;
        }
        dup3_result = dispatch(abi, HXNU_SYS_DUP3, [fd, 63, O_CLOEXEC, 0, 0, 0]).value;
        if dup3_result >= 0 {
            let _ = dispatch(abi, HXNU_SYS_CLOSE, [dup3_result as u64, 0, 0, 0, 0, 0]).value;
        }
        seek_result = dispatch(abi, HXNU_SYS_SEEK, [fd, 0, SEEK_SET as u64, 0, 0, 0]).value;
        close_result = dispatch(abi, HXNU_SYS_CLOSE, [fd, 0, 0, 0, 0, 0]).value;
    }
    let open_dir_result =
        dispatch(abi, HXNU_SYS_OPEN, [OPEN_DIR_PATH.as_ptr() as u64, 0, 0, 0, 0, 0]).value;
    if open_dir_result >= 0 {
        let fd = open_dir_result as u64;
        fchdir_result = dispatch(abi, HXNU_SYS_FCHDIR, [fd, 0, 0, 0, 0, 0]).value;
        let mut dir_buffer = [0u8; 256];
        getdents_result = dispatch(
            abi,
            HXNU_SYS_GETDENTS,
            [fd, dir_buffer.as_mut_ptr() as u64, dir_buffer.len() as u64, 0, 0, 0],
        )
        .value;
        let _ = dispatch(abi, HXNU_SYS_CLOSE, [fd, 0, 0, 0, 0, 0]).value;
    }
    let _ = dispatch(abi, HXNU_SYS_CHDIR, [ROOT_PATH.as_ptr() as u64, 0, 0, 0, 0, 0]).value;

    let process_self_result = dispatch(abi, HXNU_SYS_PROCESS_SELF, [0; 6]).value;
    let process_parent_result = dispatch(abi, HXNU_SYS_PROCESS_PARENT, [0; 6]).value;
    let thread_self_result = dispatch(abi, HXNU_SYS_THREAD_SELF, [0; 6]).value;
    let umask_result = dispatch(abi, HXNU_SYS_UMASK, [0o077, 0, 0, 0, 0, 0]).value;
    let umask_restore_value = if umask_result >= 0 {
        umask_result as u64
    } else {
        DEFAULT_PROCESS_UMASK as u64
    };
    let umask_restore_result = dispatch(abi, HXNU_SYS_UMASK, [umask_restore_value, 0, 0, 0, 0, 0]).value;
    let getuid_result = dispatch(abi, HXNU_SYS_GETUID, [0; 6]).value;
    let getgid_result = dispatch(abi, HXNU_SYS_GETGID, [0; 6]).value;
    let geteuid_result = dispatch(abi, HXNU_SYS_GETEUID, [0; 6]).value;
    let getegid_result = dispatch(abi, HXNU_SYS_GETEGID, [0; 6]).value;
    let mut clear_tid_slot = 0i32;
    let set_tid_address_result = dispatch(
        abi,
        HXNU_SYS_SET_TID_ADDRESS,
        [(&mut clear_tid_slot as *mut i32) as u64, 0, 0, 0, 0, 0],
    )
    .value;
    let clear_tid_snapshot = i64::from(clear_tid_slot);
    let sched_yield_result = dispatch(abi, HXNU_SYS_SCHED_YIELD, [0; 6]).value;
    let uptime_result = dispatch(abi, HXNU_SYS_UPTIME_NSEC, [0; 6]).value;
    let abi_version_result = dispatch(abi, HXNU_SYS_ABI_VERSION, [0; 6]).value;

    let exit_group = dispatch(abi, HXNU_SYS_EXIT_GROUP, [23, 0, 0, 0, 0, 0]);
    let (exit_group_captured, exit_group_status) = exit_status(exit_group);

    HxnuBootstrapProbe {
        write_result,
        open_result,
        mmap_result,
        mprotect_result,
        munmap_result,
        brk_result,
        brk_set_result,
        brk_restore_result,
        nanosleep_result,
        gettimeofday_result,
        gettimeofday_seconds: timeval.tv_sec,
        gettimeofday_microseconds: timeval.tv_usec,
        getrandom_result,
        getrandom_sample,
        rt_sigaction_result,
        rt_sigprocmask_result,
        rt_sigmask_snapshot,
        rt_sigold_handler,
        wait4_result,
        setpgid_result,
        getpgid_result,
        setsid_result,
        getsid_result,
        getrlimit_result,
        setrlimit_result,
        prlimit64_result,
        prctl_set_name_result,
        prctl_get_name_result,
        prctl_set_dumpable_result,
        prctl_get_dumpable_result,
        set_robust_list_result,
        get_robust_list_result,
        rseq_register_result,
        rseq_unregister_result,
        pread64_result,
        pwrite64_result,
        readv_result,
        writev_result,
        ioctl_result,
        access_result,
        stat_result,
        readlink_result,
        dup_result,
        dup2_result,
        dup3_result,
        fcntl_getfd_result,
        fcntl_getfl_result,
        getcwd_result,
        chdir_result,
        fchdir_result,
        read_result,
        fstat_result,
        getdents_result,
        seek_result,
        close_result,
        process_self_result,
        process_parent_result,
        thread_self_result,
        umask_result,
        umask_restore_result,
        getuid_result,
        getgid_result,
        geteuid_result,
        getegid_result,
        set_tid_address_result,
        clear_tid_snapshot,
        sched_yield_result,
        uptime_result,
        abi_version_result,
        exit_group_captured,
        exit_group_status,
    }
}

fn linux_openat(args: [u64; 6]) -> SyscallOutcome {
    let dirfd = args[0] as i64;
    open_path_at(dirfd, args[1] as usize, args[2])
}

fn linux_mmap(args: [u64; 6]) -> SyscallOutcome {
    process_mmap(args)
}

fn linux_getcwd(args: [u64; 6]) -> SyscallOutcome {
    process_getcwd(args)
}

fn linux_chdir(args: [u64; 6]) -> SyscallOutcome {
    process_chdir(args)
}

fn linux_fchdir(args: [u64; 6]) -> SyscallOutcome {
    process_fchdir(args)
}

fn linux_newfstatat(args: [u64; 6]) -> SyscallOutcome {
    let dirfd = args[0] as i64;
    stat_path_at(dirfd, args[1] as usize, args[2], args[3])
}

fn linux_faccessat(args: [u64; 6]) -> SyscallOutcome {
    let dirfd = args[0] as i64;
    access_path_at(dirfd, args[1] as usize, args[2], args[3])
}

fn linux_faccessat2(args: [u64; 6]) -> SyscallOutcome {
    let dirfd = args[0] as i64;
    access_path_at(dirfd, args[1] as usize, args[2], args[3])
}

fn linux_readlinkat(args: [u64; 6]) -> SyscallOutcome {
    let dirfd = args[0] as i64;
    readlink_path_at(dirfd, args[1] as usize, args[2] as usize, args[3])
}

fn process_getcwd(args: [u64; 6]) -> SyscallOutcome {
    let destination_ptr = args[0] as usize;
    let buffer_len = match usize::try_from(args[1]) {
        Ok(length) => length,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if buffer_len == 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    let cwd = current_working_directory_path();
    let bytes = cwd.as_bytes();
    let required = bytes.len().saturating_add(1);
    if required > buffer_len {
        return SyscallOutcome::errno(ERANGE);
    }

    let mut output = Vec::with_capacity(required);
    output.extend_from_slice(bytes);
    output.push(0);
    if let Err(error) = uaccess::copyout(&output, destination_ptr) {
        return SyscallOutcome::errno(map_uaccess_error(error));
    }

    match i64::try_from(required) {
        Ok(required) => SyscallOutcome::success(required),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn process_chdir(args: [u64; 6]) -> SyscallOutcome {
    match change_directory_at(AT_FDCWD, args[0] as usize) {
        Ok(value) => SyscallOutcome::success(value),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn process_fchdir(args: [u64; 6]) -> SyscallOutcome {
    let fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    match change_directory_by_fd(fd) {
        Ok(value) => SyscallOutcome::success(value),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn process_mmap(args: [u64; 6]) -> SyscallOutcome {
    let length = match usize::try_from(args[1]) {
        Ok(length) => length,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    let prot = args[2];
    let flags = args[3];
    let offset = args[5];

    if length == 0 {
        return SyscallOutcome::errno(EINVAL);
    }
    if prot != PROT_NONE && prot & !PROT_MASK != 0 {
        return SyscallOutcome::errno(EINVAL);
    }
    if flags & (MAP_PRIVATE | MAP_SHARED) == 0 {
        return SyscallOutcome::errno(EINVAL);
    }
    if flags & MAP_FIXED != 0 {
        return SyscallOutcome::errno(EINVAL);
    }
    if flags & MAP_ANONYMOUS == 0 {
        return SyscallOutcome::errno(ENOSYS);
    }
    if offset as usize % MMAP_PAGE_SIZE != 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    match map_anonymous_region(length, prot) {
        Ok(address) => SyscallOutcome::success(address),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn process_munmap(args: [u64; 6]) -> SyscallOutcome {
    let address = args[0] as usize;
    let length = match usize::try_from(args[1]) {
        Ok(length) => length,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if length == 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    match unmap_region(address, length) {
        Ok(value) => SyscallOutcome::success(value),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn process_mprotect(args: [u64; 6]) -> SyscallOutcome {
    let address = args[0] as usize;
    let length = match usize::try_from(args[1]) {
        Ok(length) => length,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    let prot = args[2];
    if length == 0 {
        return SyscallOutcome::errno(EINVAL);
    }
    if prot != PROT_NONE && prot & !PROT_MASK != 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    match protect_region(address, length, prot) {
        Ok(value) => SyscallOutcome::success(value),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn process_brk(args: [u64; 6]) -> SyscallOutcome {
    let requested = args[0] as usize;
    let current = current_process_brk();
    if requested == 0 {
        return to_address_outcome(current);
    }
    if requested < DEFAULT_PROCESS_BRK {
        return to_address_outcome(current);
    }
    set_process_brk(requested);
    to_address_outcome(requested)
}

fn process_wait4(args: [u64; 6]) -> SyscallOutcome {
    let pid = args[0] as i64;
    let _status_ptr = args[1] as usize;
    let options = match i32::try_from(args[2]) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };
    if options & !WNOHANG != 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    let current_pid = match i64::try_from(current_process_id_value()) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    let waits_current_scope = pid == -1 || pid == 0 || pid == current_pid;
    if !waits_current_scope {
        return SyscallOutcome::errno(ECHILD);
    }
    if options & WNOHANG != 0 {
        return SyscallOutcome::success(0);
    }

    SyscallOutcome::errno(ECHILD)
}

fn process_setpgid(args: [u64; 6]) -> SyscallOutcome {
    let pid = args[0] as i64;
    let pgid = args[1] as i64;
    if pgid < 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    let current_pid = match i64::try_from(current_process_id_value()) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if pid != 0 && pid != current_pid {
        return SyscallOutcome::errno(ESRCH);
    }

    let next_pgid = if pgid == 0 {
        current_process_id_value()
    } else {
        match u64::try_from(pgid) {
            Ok(value) => value,
            Err(_) => return SyscallOutcome::errno(ERANGE),
        }
    };
    set_process_group_id(next_pgid);
    SyscallOutcome::success(0)
}

fn process_getpgid(args: [u64; 6]) -> SyscallOutcome {
    let pid = args[0] as i64;
    let current_pid = match i64::try_from(current_process_id_value()) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if pid != 0 && pid != current_pid {
        return SyscallOutcome::errno(ESRCH);
    }

    let pgid = current_process_group_id();
    match i64::try_from(pgid) {
        Ok(value) => SyscallOutcome::success(value),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn process_setsid() -> SyscallOutcome {
    let sid = current_process_id_value();
    set_session_and_group_id(sid, sid);
    match i64::try_from(sid) {
        Ok(value) => SyscallOutcome::success(value),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn process_getsid(args: [u64; 6]) -> SyscallOutcome {
    let pid = args[0] as i64;
    let current_pid = match i64::try_from(current_process_id_value()) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if pid != 0 && pid != current_pid {
        return SyscallOutcome::errno(ESRCH);
    }

    let sid = current_session_id();
    match i64::try_from(sid) {
        Ok(value) => SyscallOutcome::success(value),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn process_getrlimit(args: [u64; 6]) -> SyscallOutcome {
    let resource = match u32::try_from(args[0]) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };
    let limit_ptr = args[1] as usize;
    if limit_ptr == 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    let limit = match current_process_rlimit(resource) {
        Some(limit) => limit,
        None => return SyscallOutcome::errno(EINVAL),
    };
    if let Err(error) = copyout_struct(limit_ptr, &limit) {
        return SyscallOutcome::errno(error);
    }

    SyscallOutcome::success(0)
}

fn process_setrlimit(args: [u64; 6]) -> SyscallOutcome {
    let resource = match u32::try_from(args[0]) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };
    let new_limit_ptr = args[1] as usize;
    if new_limit_ptr == 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    let next_limit = match copyin_rlimit(new_limit_ptr) {
        Ok(limit) => limit,
        Err(error) => return SyscallOutcome::errno(error),
    };
    if let Err(error) = validate_rlimit_update(resource, next_limit) {
        return SyscallOutcome::errno(error);
    }
    set_process_rlimit(resource, next_limit);
    SyscallOutcome::success(0)
}

fn process_prlimit64(args: [u64; 6]) -> SyscallOutcome {
    let pid = args[0] as i64;
    let resource = match u32::try_from(args[1]) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };
    let new_limit_ptr = args[2] as usize;
    let old_limit_ptr = args[3] as usize;

    let current_pid = match i64::try_from(current_process_id_value()) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if pid != 0 && pid != current_pid {
        return SyscallOutcome::errno(ESRCH);
    }

    let current_limit = match current_process_rlimit(resource) {
        Some(limit) => limit,
        None => return SyscallOutcome::errno(EINVAL),
    };
    if old_limit_ptr != 0 {
        if let Err(error) = copyout_struct(old_limit_ptr, &current_limit) {
            return SyscallOutcome::errno(error);
        }
    }
    if new_limit_ptr == 0 {
        return SyscallOutcome::success(0);
    }

    let next_limit = match copyin_rlimit(new_limit_ptr) {
        Ok(limit) => limit,
        Err(error) => return SyscallOutcome::errno(error),
    };
    if let Err(error) = validate_rlimit_update(resource, next_limit) {
        return SyscallOutcome::errno(error);
    }
    set_process_rlimit(resource, next_limit);
    SyscallOutcome::success(0)
}

fn process_prctl(args: [u64; 6]) -> SyscallOutcome {
    let option = match i32::try_from(args[0]) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };
    match option {
        PR_SET_DUMPABLE => {
            let value = match i32::try_from(args[1]) {
                Ok(value) => value,
                Err(_) => return SyscallOutcome::errno(EINVAL),
            };
            if value != 0 && value != 1 {
                return SyscallOutcome::errno(EINVAL);
            }
            set_process_dumpable(value);
            SyscallOutcome::success(0)
        }
        PR_GET_DUMPABLE => SyscallOutcome::success(i64::from(current_process_dumpable())),
        PR_SET_NAME => {
            let name_ptr = args[1] as usize;
            if name_ptr == 0 {
                return SyscallOutcome::errno(EINVAL);
            }
            let name = match copyin_comm_name(name_ptr) {
                Ok(name) => name,
                Err(error) => return SyscallOutcome::errno(error),
            };
            set_process_comm_name(name);
            SyscallOutcome::success(0)
        }
        PR_GET_NAME => {
            let name_ptr = args[1] as usize;
            if name_ptr == 0 {
                return SyscallOutcome::errno(EINVAL);
            }
            let name = current_process_comm_name();
            if let Err(error) = copyout_struct(name_ptr, &name) {
                return SyscallOutcome::errno(error);
            }
            SyscallOutcome::success(0)
        }
        _ => SyscallOutcome::errno(EINVAL),
    }
}

fn process_set_robust_list(args: [u64; 6]) -> SyscallOutcome {
    let head = args[0] as usize;
    let len = match usize::try_from(args[1]) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if len != size_of::<LinuxRobustListHead>() {
        return SyscallOutcome::errno(EINVAL);
    }

    set_process_robust_list(head, len);
    SyscallOutcome::success(0)
}

fn process_get_robust_list(args: [u64; 6]) -> SyscallOutcome {
    let pid = args[0] as i64;
    let head_ptr_ptr = args[1] as usize;
    let len_ptr = args[2] as usize;
    if head_ptr_ptr == 0 || len_ptr == 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    let current_pid = match i64::try_from(current_process_id_value()) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if pid != 0 && pid != current_pid {
        return SyscallOutcome::errno(ESRCH);
    }

    let (head, len) = current_process_robust_list();
    let head_u64 = match u64::try_from(head) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if let Err(error) = copyout_struct(head_ptr_ptr, &head_u64) {
        return SyscallOutcome::errno(error);
    }
    if let Err(error) = copyout_struct(len_ptr, &len) {
        return SyscallOutcome::errno(error);
    }
    SyscallOutcome::success(0)
}

fn process_rseq(args: [u64; 6]) -> SyscallOutcome {
    let address = args[0] as usize;
    let length = match u32::try_from(args[1]) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };
    let flags = match u32::try_from(args[2]) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };
    let signature = match u32::try_from(args[3]) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };
    if length != size_of::<LinuxRseqArea>() as u32 {
        return SyscallOutcome::errno(EINVAL);
    }
    if flags & !RSEQ_FLAG_UNREGISTER != 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    if flags == RSEQ_FLAG_UNREGISTER {
        let current = current_process_rseq_state();
        if !current.registered
            || current.address != address
            || current.length != length
            || current.signature != signature
        {
            return SyscallOutcome::errno(EINVAL);
        }
        clear_process_rseq_state();
        return SyscallOutcome::success(0);
    }

    if address == 0 {
        return SyscallOutcome::errno(EINVAL);
    }
    set_process_rseq_state(ProcessRseqState {
        process_id: current_process_id_value(),
        address,
        length,
        signature,
        registered: true,
    });
    SyscallOutcome::success(0)
}

fn process_rt_sigprocmask(args: [u64; 6]) -> SyscallOutcome {
    let how = match i32::try_from(args[0]) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };
    let set_ptr = args[1] as usize;
    let oldset_ptr = args[2] as usize;
    let sigset_size = match usize::try_from(args[3]) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if sigset_size != RT_SIGSET_SIZE {
        return SyscallOutcome::errno(EINVAL);
    }

    let current = current_process_signal_mask();
    if oldset_ptr != 0 {
        if let Err(error) = copyout_struct(oldset_ptr, &current) {
            return SyscallOutcome::errno(error);
        }
    }
    if set_ptr == 0 {
        return SyscallOutcome::success(0);
    }

    let set = match copyin_sigset(set_ptr, sigset_size) {
        Ok(mask) => mask,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let next = match how {
        SIG_BLOCK => current | set,
        SIG_UNBLOCK => current & !set,
        SIG_SETMASK => set,
        _ => return SyscallOutcome::errno(EINVAL),
    };
    set_process_signal_mask(next);

    SyscallOutcome::success(0)
}

fn process_rt_sigaction(args: [u64; 6]) -> SyscallOutcome {
    let signum = args[0];
    if signum == 0 || signum > MAX_SIGNAL_NUMBER {
        return SyscallOutcome::errno(EINVAL);
    }
    let sigset_size = match usize::try_from(args[3]) {
        Ok(value) => value,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if sigset_size != RT_SIGSET_SIZE {
        return SyscallOutcome::errno(EINVAL);
    }
    let new_action_ptr = args[1] as usize;
    let old_action_ptr = args[2] as usize;

    if old_action_ptr != 0 {
        let current = current_signal_action(signum as u8);
        if let Err(error) = copyout_struct(old_action_ptr, &current) {
            return SyscallOutcome::errno(error);
        }
    }
    if new_action_ptr == 0 {
        return SyscallOutcome::success(0);
    }
    if signum == SIGKILL || signum == SIGSTOP {
        return SyscallOutcome::errno(EINVAL);
    }

    let action = match copyin_sigaction(new_action_ptr) {
        Ok(action) => action,
        Err(error) => return SyscallOutcome::errno(error),
    };
    set_signal_action(signum as u8, action);
    SyscallOutcome::success(0)
}

fn process_pread64(args: [u64; 6]) -> SyscallOutcome {
    let fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let count = match usize::try_from(args[2]) {
        Ok(count) => count,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if count > MAX_READ_BYTES {
        return SyscallOutcome::errno(ERANGE);
    }
    let offset = args[3] as i64;
    if offset < 0 {
        return SyscallOutcome::errno(EINVAL);
    }
    if count == 0 {
        return SyscallOutcome::success(0);
    }

    match read_open_file_at_offset(fd, args[1] as usize, count, offset) {
        Ok(value) => SyscallOutcome::success(value),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn process_pwrite64(args: [u64; 6]) -> SyscallOutcome {
    let fd = args[0];
    let offset = args[3] as i64;
    if offset < 0 {
        return SyscallOutcome::errno(EINVAL);
    }
    if fd == STDOUT_FD || fd == STDERR_FD {
        return write_text(args[1] as usize, args[2]);
    }

    SyscallOutcome::errno(EROFS)
}

fn process_readv(args: [u64; 6]) -> SyscallOutcome {
    let fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let iov_ptr = args[1] as usize;
    let iovcnt = match usize::try_from(args[2]) {
        Ok(count) => count,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if iovcnt == 0 {
        return SyscallOutcome::success(0);
    }
    if iovcnt > MAX_IOVEC_COUNT {
        return SyscallOutcome::errno(EINVAL);
    }

    let mut total = 0usize;
    for index in 0..iovcnt {
        let iov = match copyin_iovec_at(iov_ptr, index) {
            Ok(iov) => iov,
            Err(error) => return SyscallOutcome::errno(error),
        };
        let len = match usize::try_from(iov.iov_len) {
            Ok(len) => len,
            Err(_) => return SyscallOutcome::errno(ERANGE),
        };
        if len > MAX_READ_BYTES {
            return SyscallOutcome::errno(ERANGE);
        }
        if len == 0 {
            continue;
        }

        let segment = match read_open_file(fd, iov.iov_base as usize, len) {
            Ok(value) => value,
            Err(error) => {
                if total == 0 {
                    return SyscallOutcome::errno(error);
                }
                return match i64::try_from(total) {
                    Ok(value) => SyscallOutcome::success(value),
                    Err(_) => SyscallOutcome::errno(ERANGE),
                };
            }
        };
        let read_len = match usize::try_from(segment) {
            Ok(value) => value,
            Err(_) => return SyscallOutcome::errno(ERANGE),
        };
        total = match total.checked_add(read_len) {
            Some(value) => value,
            None => return SyscallOutcome::errno(ERANGE),
        };
        if read_len < len {
            break;
        }
    }

    match i64::try_from(total) {
        Ok(value) => SyscallOutcome::success(value),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn process_writev(args: [u64; 6]) -> SyscallOutcome {
    let fd = args[0];
    if fd != STDOUT_FD && fd != STDERR_FD {
        return SyscallOutcome::errno(EBADF);
    }

    let iov_ptr = args[1] as usize;
    let iovcnt = match usize::try_from(args[2]) {
        Ok(count) => count,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if iovcnt == 0 {
        return SyscallOutcome::success(0);
    }
    if iovcnt > MAX_IOVEC_COUNT {
        return SyscallOutcome::errno(EINVAL);
    }

    let mut total = 0usize;
    for index in 0..iovcnt {
        let iov = match copyin_iovec_at(iov_ptr, index) {
            Ok(iov) => iov,
            Err(error) => return SyscallOutcome::errno(error),
        };
        let len = match usize::try_from(iov.iov_len) {
            Ok(len) => len,
            Err(_) => return SyscallOutcome::errno(ERANGE),
        };
        if len == 0 {
            continue;
        }
        let next_total = match total.checked_add(len) {
            Some(value) => value,
            None => return SyscallOutcome::errno(ERANGE),
        };
        if next_total > MAX_WRITE_BYTES {
            return SyscallOutcome::errno(ERANGE);
        }

        let bytes = match copyin_bytes(iov.iov_base as usize, len) {
            Ok(bytes) => bytes,
            Err(error) => {
                if total == 0 {
                    return SyscallOutcome::errno(error);
                }
                return match i64::try_from(total) {
                    Ok(value) => SyscallOutcome::success(value),
                    Err(_) => SyscallOutcome::errno(ERANGE),
                };
            }
        };
        tty::write_str(&sanitize_for_console(&bytes));
        total = next_total;
    }

    match i64::try_from(total) {
        Ok(value) => SyscallOutcome::success(value),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn process_nanosleep(args: [u64; 6]) -> SyscallOutcome {
    let request_ptr = args[0] as usize;
    let remainder_ptr = args[1] as usize;
    if request_ptr == 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    if remainder_ptr != 0 {
        let remainder = LinuxTimespec { tv_sec: 0, tv_nsec: 0 };
        if let Err(error) = copyout_struct(remainder_ptr, &remainder) {
            return SyscallOutcome::errno(error);
        }
    }

    SyscallOutcome::success(0)
}

fn process_gettimeofday(args: [u64; 6]) -> SyscallOutcome {
    let timeval_ptr = args[0] as usize;
    if timeval_ptr == 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    let uptime_ns = time::uptime_nanoseconds();
    let seconds = uptime_ns / 1_000_000_000;
    let microseconds = (uptime_ns % 1_000_000_000) / 1_000;
    let timeval = LinuxTimeval {
        tv_sec: match i64::try_from(seconds) {
            Ok(value) => value,
            Err(_) => return SyscallOutcome::errno(ERANGE),
        },
        tv_usec: match i64::try_from(microseconds) {
            Ok(value) => value,
            Err(_) => return SyscallOutcome::errno(ERANGE),
        },
    };
    if let Err(error) = copyout_struct(timeval_ptr, &timeval) {
        return SyscallOutcome::errno(error);
    }

    SyscallOutcome::success(0)
}

fn process_getrandom(args: [u64; 6]) -> SyscallOutcome {
    let destination_ptr = args[0] as usize;
    let count = match usize::try_from(args[1]) {
        Ok(count) => count,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    let flags = args[2];
    if flags & !GRND_MASK != 0 {
        return SyscallOutcome::errno(EINVAL);
    }
    if count > MAX_READ_BYTES {
        return SyscallOutcome::errno(ERANGE);
    }
    if count == 0 {
        return SyscallOutcome::success(0);
    }

    let mut buffer = vec![0u8; count];
    fill_pseudo_random_bytes(&mut buffer);
    if let Err(error) = uaccess::copyout(&buffer, destination_ptr) {
        return SyscallOutcome::errno(map_uaccess_error(error));
    }

    match i64::try_from(count) {
        Ok(value) => SyscallOutcome::success(value),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn open_path_at(dirfd: i64, path_ptr: usize, flags: u64) -> SyscallOutcome {
    if !is_read_only_open(flags) {
        return SyscallOutcome::errno(EINVAL);
    }

    let node = match lookup_node_at(dirfd, path_ptr) {
        Ok(node) => node,
        Err(error) => return SyscallOutcome::errno(error),
    };

    let content = match vfs::read(&node.path) {
        Some(content) => content.into_bytes(),
        None => return SyscallOutcome::errno(EIO),
    };
    match alloc_open_file(node.path, node.mount, node.kind, node.executable, content) {
        Ok(fd) => SyscallOutcome::success(fd),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn stat_path_at(dirfd: i64, path_ptr: usize, stat_ptr: u64, flags: u64) -> SyscallOutcome {
    if flags != 0 {
        return SyscallOutcome::errno(EINVAL);
    }
    let stat_ptr = stat_ptr as usize;
    let node = match lookup_node_at(dirfd, path_ptr) {
        Ok(node) => node,
        Err(error) => return SyscallOutcome::errno(error),
    };

    let stat = match build_linux_stat(node.mount, node.kind, node.executable, node.size, &node.path) {
        Ok(stat) => stat,
        Err(error) => return SyscallOutcome::errno(error),
    };
    if let Err(error) = copyout_struct(stat_ptr, &stat) {
        return SyscallOutcome::errno(error);
    }

    SyscallOutcome::success(0)
}

fn access_path_at(dirfd: i64, path_ptr: usize, mode: u64, flags: u64) -> SyscallOutcome {
    if flags & !AT_EACCESS != 0 {
        return SyscallOutcome::errno(EINVAL);
    }
    if mode & !(R_OK | W_OK | X_OK | F_OK) != 0 {
        return SyscallOutcome::errno(EINVAL);
    }

    let node = match lookup_node_at(dirfd, path_ptr) {
        Ok(node) => node,
        Err(error) => return SyscallOutcome::errno(error),
    };
    if mode == F_OK {
        return SyscallOutcome::success(0);
    }
    if mode & W_OK != 0 {
        return SyscallOutcome::errno(EACCES);
    }
    if mode & X_OK != 0 && !is_executable_node(node.kind, node.executable) {
        return SyscallOutcome::errno(EACCES);
    }

    SyscallOutcome::success(0)
}

fn readlink_path_at(dirfd: i64, path_ptr: usize, destination_ptr: usize, count: u64) -> SyscallOutcome {
    let count = match usize::try_from(count) {
        Ok(count) => count,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if count > MAX_READ_BYTES {
        return SyscallOutcome::errno(ERANGE);
    }
    if count == 0 {
        return SyscallOutcome::success(0);
    }

    let resolved_path = match resolve_path_at(dirfd, path_ptr) {
        Ok(path) => path,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let target = match readlink_target_for_path(&resolved_path) {
        Ok(target) => target,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let target_bytes = target.as_bytes();
    let copy_len = min(target_bytes.len(), count);
    if let Err(error) = uaccess::copyout(&target_bytes[..copy_len], destination_ptr) {
        return SyscallOutcome::errno(map_uaccess_error(error));
    }

    match i64::try_from(copy_len) {
        Ok(copy_len) => SyscallOutcome::success(copy_len),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn lookup_node_at(dirfd: i64, path_ptr: usize) -> Result<vfs::VfsNode, i64> {
    let resolved_path = resolve_path_at(dirfd, path_ptr)?;
    vfs::lookup(&resolved_path).ok_or(ENOENT)
}

fn resolve_path_at(dirfd: i64, path_ptr: usize) -> Result<String, i64> {
    let raw_path = copyin_c_string(path_ptr, MAX_PATH_BYTES)?;
    if raw_path.is_empty() {
        return Err(EINVAL);
    }

    if raw_path.starts_with('/') {
        return Ok(raw_path);
    }

    let mut base = base_path_for_dirfd(dirfd)?;
    if !base.ends_with('/') {
        base.push('/');
    }
    base.push_str(&raw_path);
    Ok(base)
}

fn base_path_for_dirfd(dirfd: i64) -> Result<String, i64> {
    if dirfd == AT_FDCWD {
        return Ok(current_working_directory_path());
    }

    let fd = i32::try_from(dirfd).map_err(|_| EBADF)?;
    let (path, kind) = open_file_path_and_kind_for_process(fd)?;
    if kind != VfsNodeKind::Directory {
        return Err(ENOTDIR);
    }

    Ok(path)
}

fn change_directory_at(dirfd: i64, path_ptr: usize) -> Result<i64, i64> {
    let resolved = resolve_path_at(dirfd, path_ptr)?;
    let node = vfs::lookup(&resolved).ok_or(ENOENT)?;
    if node.kind != VfsNodeKind::Directory {
        return Err(ENOTDIR);
    }

    set_working_directory_path(node.path);
    Ok(0)
}

fn change_directory_by_fd(fd: i32) -> Result<i64, i64> {
    let (path, kind) = open_file_path_and_kind_for_process(fd)?;
    if kind != VfsNodeKind::Directory {
        return Err(ENOTDIR);
    }

    set_working_directory_path(path);
    Ok(0)
}

fn readlink_target_for_path(path: &str) -> Result<String, i64> {
    if path == "/proc/self/exe" {
        return Ok(String::from("/initrd/init"));
    }

    if vfs::lookup(path).is_some() {
        return Err(EINVAL);
    }
    Err(ENOENT)
}

fn read_from_fd(args: [u64; 6]) -> SyscallOutcome {
    let fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };

    let count = match usize::try_from(args[2]) {
        Ok(count) => count,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if count > MAX_READ_BYTES {
        return SyscallOutcome::errno(ERANGE);
    }

    match read_open_file(fd, args[1] as usize, count) {
        Ok(read) => SyscallOutcome::success(read),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn close_fd(args: [u64; 6]) -> SyscallOutcome {
    let fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    match close_open_file(fd) {
        Ok(value) => SyscallOutcome::success(value),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn dup_fd(args: [u64; 6]) -> SyscallOutcome {
    let source_fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };

    match duplicate_fd(source_fd, 3, DuplicateTarget::LowestAvailable, 0) {
        Ok(value) => SyscallOutcome::success(value),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn dup2_fd(args: [u64; 6]) -> SyscallOutcome {
    let source_fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let target_fd = match parse_fd(args[1]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    if target_fd < 3 {
        return SyscallOutcome::errno(EBADF);
    }
    if source_fd == target_fd {
        return SyscallOutcome::success(i64::from(target_fd));
    }

    match duplicate_fd(source_fd, target_fd, DuplicateTarget::Exact, 0) {
        Ok(value) => SyscallOutcome::success(value),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn dup3_fd(args: [u64; 6]) -> SyscallOutcome {
    let source_fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let target_fd = match parse_fd(args[1]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    if source_fd == target_fd {
        return SyscallOutcome::errno(EINVAL);
    }
    if target_fd < 3 {
        return SyscallOutcome::errno(EBADF);
    }
    let flags = args[2];
    if flags & !O_CLOEXEC != 0 {
        return SyscallOutcome::errno(EINVAL);
    }
    let fd_flags = if flags & O_CLOEXEC != 0 { FD_CLOEXEC } else { 0 };

    match duplicate_fd(source_fd, target_fd, DuplicateTarget::Exact, fd_flags) {
        Ok(value) => SyscallOutcome::success(value),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn fcntl_fd(args: [u64; 6]) -> SyscallOutcome {
    let fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let command = match i32::try_from(args[1]) {
        Ok(command) => command,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };

    match command {
        F_DUPFD => {
            let minimum_fd = match parse_fd(args[2]) {
                Ok(fd) => fd.max(3),
                Err(error) => return SyscallOutcome::errno(error),
            };
            match duplicate_fd(fd, minimum_fd, DuplicateTarget::LowestAvailable, 0) {
                Ok(value) => SyscallOutcome::success(value),
                Err(error) => SyscallOutcome::errno(error),
            }
        }
        F_GETFD => match get_descriptor_flags(fd) {
            Ok(flags) => SyscallOutcome::success(i64::from(flags as i32)),
            Err(error) => SyscallOutcome::errno(error),
        },
        F_SETFD => {
            let flags = (args[2] as u32) & FD_CLOEXEC;
            match set_descriptor_flags(fd, flags) {
                Ok(value) => SyscallOutcome::success(value),
                Err(error) => SyscallOutcome::errno(error),
            }
        }
        F_GETFL => match get_status_flags(fd) {
            Ok(flags) => SyscallOutcome::success(flags),
            Err(error) => SyscallOutcome::errno(error),
        },
        F_SETFL => SyscallOutcome::success(0),
        _ => SyscallOutcome::errno(EINVAL),
    }
}

fn fstat_fd(args: [u64; 6]) -> SyscallOutcome {
    let fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let stat_ptr = args[1] as usize;
    match stat_open_file(fd, stat_ptr) {
        Ok(value) => SyscallOutcome::success(value),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn ioctl_fd(args: [u64; 6]) -> SyscallOutcome {
    let fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let request = args[1];
    let arg_ptr = args[2] as usize;

    match request {
        LINUX_TIOCGWINSZ => match ioctl_get_winsize(fd, arg_ptr) {
            Ok(value) => SyscallOutcome::success(value),
            Err(error) => SyscallOutcome::errno(error),
        },
        _ => SyscallOutcome::errno(ENOTTY),
    }
}

fn getdents_fd(args: [u64; 6]) -> SyscallOutcome {
    let fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let count = match usize::try_from(args[2]) {
        Ok(count) => count,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if count > MAX_READ_BYTES {
        return SyscallOutcome::errno(ERANGE);
    }
    if count == 0 {
        return SyscallOutcome::success(0);
    }

    match read_directory_entries(fd, args[1] as usize, count) {
        Ok(value) => SyscallOutcome::success(value),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn seek_fd(args: [u64; 6]) -> SyscallOutcome {
    let fd = match parse_fd(args[0]) {
        Ok(fd) => fd,
        Err(error) => return SyscallOutcome::errno(error),
    };
    let whence = match i32::try_from(args[2]) {
        Ok(whence) => whence,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };
    let offset = args[1] as i64;
    match seek_open_file(fd, offset, whence) {
        Ok(position) => SyscallOutcome::success(position),
        Err(error) => SyscallOutcome::errno(error),
    }
}

fn write_with_fd(args: [u64; 6]) -> SyscallOutcome {
    let fd = args[0];
    if fd != STDOUT_FD && fd != STDERR_FD {
        return SyscallOutcome::errno(EBADF);
    }

    write_text(args[1] as usize, args[2])
}

fn write_without_fd(args: [u64; 6]) -> SyscallOutcome {
    write_text(args[0] as usize, args[1])
}

fn write_text(ptr: usize, len: u64) -> SyscallOutcome {
    let count = match usize::try_from(len) {
        Ok(count) => count,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if count > MAX_WRITE_BYTES {
        return SyscallOutcome::errno(ERANGE);
    }
    if count == 0 {
        return SyscallOutcome::success(0);
    }

    let bytes = match copyin_bytes(ptr, count) {
        Ok(bytes) => bytes,
        Err(error) => return SyscallOutcome::errno(error),
    };

    let text = sanitize_for_console(&bytes);
    tty::write_str(&text);

    match i64::try_from(count) {
        Ok(written) => SyscallOutcome::success(written),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn process_id() -> SyscallOutcome {
    match i64::try_from(current_process_id_value()) {
        Ok(process_id) => SyscallOutcome::success(process_id),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn process_parent_id() -> SyscallOutcome {
    let parent_process_id = sched::stats().current_parent_process_id;
    match i64::try_from(parent_process_id) {
        Ok(parent_process_id) => SyscallOutcome::success(parent_process_id),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn thread_id() -> SyscallOutcome {
    let tid = sched::stats().current_thread_id;
    match i64::try_from(tid) {
        Ok(tid) => SyscallOutcome::success(tid),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn process_umask(args: [u64; 6]) -> SyscallOutcome {
    let requested = match u32::try_from(args[0]) {
        Ok(mask) => mask & UMASK_MODE_MASK,
        Err(_) => return SyscallOutcome::errno(EINVAL),
    };
    let previous = set_process_umask_value(requested);
    SyscallOutcome::success(i64::from(previous))
}

fn user_id() -> SyscallOutcome {
    SyscallOutcome::success(0)
}

fn group_id() -> SyscallOutcome {
    SyscallOutcome::success(0)
}

fn set_tid_address(args: [u64; 6]) -> SyscallOutcome {
    let clear_tid_address = args[0] as usize;
    set_process_clear_tid_address(clear_tid_address);

    let tid = sched::stats().current_thread_id;
    let tid_i32 = match i32::try_from(tid) {
        Ok(tid) => tid,
        Err(_) => return SyscallOutcome::errno(ERANGE),
    };
    if clear_tid_address != 0 {
        if let Err(error) = copyout_struct(clear_tid_address, &tid_i32) {
            return SyscallOutcome::errno(error);
        }
    }

    SyscallOutcome::success(i64::from(tid_i32))
}

fn uptime_ns() -> SyscallOutcome {
    let uptime = time::uptime_nanoseconds();
    match i64::try_from(uptime) {
        Ok(uptime) => SyscallOutcome::success(uptime),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn linux_clock_gettime(args: [u64; 6]) -> SyscallOutcome {
    let clock_id = args[0] as i32;
    if clock_id != LINUX_CLOCK_REALTIME && clock_id != LINUX_CLOCK_MONOTONIC {
        return SyscallOutcome::errno(EINVAL);
    }

    let ptr = args[1] as usize;
    let uptime_ns = time::uptime_nanoseconds();
    let timespec = LinuxTimespec {
        tv_sec: (uptime_ns / 1_000_000_000) as i64,
        tv_nsec: (uptime_ns % 1_000_000_000) as i64,
    };
    if let Err(error) = copyout_struct(ptr, &timespec) {
        return SyscallOutcome::errno(error);
    }

    SyscallOutcome::success(0)
}

fn linux_uname(args: [u64; 6]) -> SyscallOutcome {
    write_uname(
        args[0] as usize,
        "Linux",
        "hxnu",
        "0.1.0-hxnu",
        "HXNU micro-hybrid kernel bootstrap",
        "x86_64",
        "localdomain",
    )
}

fn ghost_uname(args: [u64; 6]) -> SyscallOutcome {
    write_uname(
        args[0] as usize,
        "Ghost",
        "hxnu",
        "0.1.0-ghost",
        "HXNU ghost compatibility bootstrap",
        "x86_64",
        "legacy",
    )
}

fn write_uname(
    ptr: usize,
    sysname: &str,
    nodename: &str,
    release: &str,
    version: &str,
    machine: &str,
    domainname: &str,
) -> SyscallOutcome {
    let mut uts = LinuxUtsName::new();
    write_uts_field(&mut uts.sysname, sysname);
    write_uts_field(&mut uts.nodename, nodename);
    write_uts_field(&mut uts.release, release);
    write_uts_field(&mut uts.version, version);
    write_uts_field(&mut uts.machine, machine);
    write_uts_field(&mut uts.domainname, domainname);
    if let Err(error) = copyout_struct(ptr, &uts) {
        return SyscallOutcome::errno(error);
    }

    SyscallOutcome::success(0)
}

fn exit_group(args: [u64; 6]) -> SyscallOutcome {
    let status = args[0] as i32;
    let process_id = current_process_id_value();
    purge_open_files_for_process(process_id);
    purge_working_directory_for_process(process_id);
    purge_process_umask(process_id);
    purge_process_clear_tid_address(process_id);
    purge_process_mappings(process_id);
    purge_process_brk(process_id);
    purge_process_group_state(process_id);
    purge_process_rlimits(process_id);
    purge_process_prctl_state(process_id);
    purge_process_robust_list_state(process_id);
    purge_process_rseq_state(process_id);
    purge_process_signal_mask(process_id);
    purge_process_signal_actions(process_id);
    SyscallOutcome {
        value: 0,
        action: SyscallAction::ExitGroup { status },
    }
}

fn exit_status(outcome: SyscallOutcome) -> (bool, i32) {
    match outcome.action {
        SyscallAction::ExitGroup { status } => (true, status),
        SyscallAction::Continue => (false, 0),
    }
}

fn build_linux_stat(
    mount: VfsMountKind,
    kind: VfsNodeKind,
    executable: bool,
    size: usize,
    path: &str,
) -> Result<LinuxStat, i64> {
    let size_i64 = i64::try_from(size).map_err(|_| ERANGE)?;
    let blocks = size.saturating_add(STAT_SECTOR_SIZE.saturating_sub(1)) / STAT_SECTOR_SIZE;
    let blocks_i64 = i64::try_from(blocks).map_err(|_| ERANGE)?;
    let inode = hash_path_to_ino(mount, path);
    let device = mount_device_id(mount);
    let mode = mode_from_node(kind, executable);
    let uptime = time::uptime_nanoseconds();
    let secs = i64::try_from(uptime / 1_000_000_000).map_err(|_| ERANGE)?;
    let nanos = i64::try_from(uptime % 1_000_000_000).map_err(|_| ERANGE)?;
    let rdev = if kind == VfsNodeKind::Device { device } else { 0 };

    Ok(LinuxStat {
        st_dev: device,
        st_ino: inode,
        st_nlink: DEFAULT_LINK_COUNT,
        st_mode: mode,
        st_uid: 0,
        st_gid: 0,
        __pad0: 0,
        st_rdev: rdev,
        st_size: size_i64,
        st_blksize: STAT_BLOCK_SIZE,
        st_blocks: blocks_i64,
        st_atime: secs,
        st_atime_nsec: nanos,
        st_mtime: secs,
        st_mtime_nsec: nanos,
        st_ctime: secs,
        st_ctime_nsec: nanos,
        __unused: [0; 3],
    })
}

fn mount_device_id(mount: VfsMountKind) -> u64 {
    match mount {
        VfsMountKind::Root => 1,
        VfsMountKind::Devfs => 2,
        VfsMountKind::Initrd => 3,
        VfsMountKind::Procfs => 4,
    }
}

fn mode_from_node(kind: VfsNodeKind, executable: bool) -> u32 {
    match kind {
        VfsNodeKind::Directory => S_IFDIR | MODE_DIRECTORY,
        VfsNodeKind::File => {
            if executable {
                S_IFREG | MODE_REGULAR_EXECUTABLE
            } else {
                S_IFREG | MODE_REGULAR_READ_ONLY
            }
        }
        VfsNodeKind::Device => S_IFCHR | MODE_CHARACTER_DEVICE,
    }
}

fn is_executable_node(kind: VfsNodeKind, executable: bool) -> bool {
    match kind {
        VfsNodeKind::Directory => true,
        VfsNodeKind::File => executable,
        VfsNodeKind::Device => false,
    }
}

fn hash_path_to_ino(mount: VfsMountKind, path: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in mount_device_id(mount).to_le_bytes().into_iter().chain(path.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    if hash == 0 { 1 } else { hash }
}

fn directory_entry_names(content: &[u8]) -> Result<Vec<&str>, i64> {
    let text = str::from_utf8(content).map_err(|_| EIO)?;
    let mut entries = Vec::new();
    for line in text.lines() {
        let name = line.trim();
        if name.is_empty() {
            continue;
        }
        entries.push(name);
    }
    Ok(entries)
}

fn directory_entry_count(content: &[u8]) -> Result<usize, i64> {
    Ok(directory_entry_names(content)?.len())
}

fn child_path_for_entry(directory_path: &str, entry_name: &str) -> String {
    if directory_path == "/" {
        let mut path = String::from("/");
        path.push_str(entry_name);
        return path;
    }

    let mut path = String::from(directory_path);
    if !path.ends_with('/') {
        path.push('/');
    }
    path.push_str(entry_name);
    path
}

fn dirent_type_from_node(kind: VfsNodeKind) -> u8 {
    match kind {
        VfsNodeKind::Directory => DT_DIR,
        VfsNodeKind::File => DT_REG,
        VfsNodeKind::Device => DT_CHR,
    }
}

fn append_linux_dirent64(
    destination: &mut [u8],
    cursor: usize,
    ino: u64,
    off: i64,
    d_type: u8,
    name: &str,
) -> Option<usize> {
    let name_bytes = name.as_bytes();
    let name_with_nul = name_bytes.len().checked_add(1)?;
    let record_len = align_to_eight(19usize.checked_add(name_with_nul)?)?;
    if record_len > u16::MAX as usize {
        return None;
    }
    let end = cursor.checked_add(record_len)?;
    if end > destination.len() {
        return None;
    }

    destination[cursor..end].fill(0);
    write_u64_le(destination, cursor, ino)?;
    write_u64_le(destination, cursor.checked_add(8)?, off as u64)?;
    write_u16_le(destination, cursor.checked_add(16)?, record_len as u16)?;
    let type_offset = cursor.checked_add(18)?;
    destination[type_offset] = d_type;
    let name_offset = cursor.checked_add(19)?;
    let name_end = name_offset.checked_add(name_bytes.len())?;
    destination.get_mut(name_offset..name_end)?.copy_from_slice(name_bytes);
    destination.get_mut(name_end).map(|byte| *byte = 0)?;
    Some(end)
}

fn align_to_eight(value: usize) -> Option<usize> {
    value.checked_add(7).map(|value| value & !7usize)
}

fn write_u64_le(destination: &mut [u8], offset: usize, value: u64) -> Option<()> {
    let end = offset.checked_add(8)?;
    destination.get_mut(offset..end)?.copy_from_slice(&value.to_le_bytes());
    Some(())
}

fn write_u16_le(destination: &mut [u8], offset: usize, value: u16) -> Option<()> {
    let end = offset.checked_add(2)?;
    destination.get_mut(offset..end)?.copy_from_slice(&value.to_le_bytes());
    Some(())
}

fn ioctl_get_winsize(fd: i32, arg_ptr: usize) -> Result<i64, i64> {
    let winsize = tty_winsize()?;

    if fd == 0 || fd as u64 == STDOUT_FD || fd as u64 == STDERR_FD {
        copyout_struct(arg_ptr, &winsize)?;
        return Ok(0);
    }

    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    let open = table
        .files
        .iter()
        .find(|file| file.fd == fd && file.owner_process_id == owner_process_id)
        .ok_or(EBADF)?;
    if open.kind != VfsNodeKind::Device || !is_tty_device_path(&open.path) {
        return Err(ENOTTY);
    }

    copyout_struct(arg_ptr, &winsize)?;
    Ok(0)
}

fn tty_winsize() -> Result<LinuxWinsize, i64> {
    let stats = tty::stats();
    let rows = u16::try_from(stats.rows).map_err(|_| ERANGE)?;
    let columns = u16::try_from(stats.columns).map_err(|_| ERANGE)?;
    Ok(LinuxWinsize {
        ws_row: rows,
        ws_col: columns,
        ws_xpixel: 0,
        ws_ypixel: 0,
    })
}

fn is_tty_device_path(path: &str) -> bool {
    path == "/dev/console" || path.starts_with("/dev/tty")
}

fn sanitize_for_console(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len());
    for &byte in bytes {
        match byte {
            b'\n' | b'\r' | b'\t' | 0x20..=0x7e => text.push(byte as char),
            _ => text.push('?'),
        }
    }
    text
}

fn is_read_only_open(flags: u64) -> bool {
    (flags & O_ACCMODE) == O_RDONLY && (flags & (O_CREAT | O_TRUNC | O_APPEND)) == 0
}

fn parse_fd(value: u64) -> Result<i32, i64> {
    i32::try_from(value).map_err(|_| EBADF)
}

fn current_process_id_value() -> u64 {
    sched::stats().current_process_id
}

fn current_working_directory_path() -> String {
    let process_id = current_process_id_value();
    let table = cwd_table_mut();
    if let Some(entry) = table.iter().find(|entry| entry.process_id == process_id) {
        return entry.path.clone();
    }

    let default = String::from("/");
    table.push(ProcessCwd {
        process_id,
        path: default.clone(),
    });
    default
}

fn set_working_directory_path(path: String) {
    let process_id = current_process_id_value();
    let table = cwd_table_mut();
    if let Some(entry) = table.iter_mut().find(|entry| entry.process_id == process_id) {
        entry.path = path;
        return;
    }

    table.push(ProcessCwd { process_id, path });
}

fn set_process_umask_value(mask: u32) -> u32 {
    let process_id = current_process_id_value();
    let table = umask_table_mut();
    if let Some(entry) = table.iter_mut().find(|entry| entry.process_id == process_id) {
        let previous = entry.mask;
        entry.mask = mask & UMASK_MODE_MASK;
        return previous;
    }

    table.push(ProcessUmask {
        process_id,
        mask: mask & UMASK_MODE_MASK,
    });
    DEFAULT_PROCESS_UMASK
}

fn set_process_clear_tid_address(address: usize) {
    let process_id = current_process_id_value();
    let table = clear_tid_table_mut();
    if let Some(entry) = table.iter_mut().find(|entry| entry.process_id == process_id) {
        entry.address = address;
        return;
    }

    table.push(ProcessClearTidAddress { process_id, address });
}

fn current_process_brk() -> usize {
    let process_id = current_process_id_value();
    let table = brk_table_mut();
    if let Some(entry) = table.iter().find(|entry| entry.process_id == process_id) {
        return entry.current_break;
    }

    table.push(ProcessBrkState {
        process_id,
        current_break: DEFAULT_PROCESS_BRK,
    });
    DEFAULT_PROCESS_BRK
}

fn set_process_brk(current_break: usize) {
    let process_id = current_process_id_value();
    let table = brk_table_mut();
    if let Some(entry) = table.iter_mut().find(|entry| entry.process_id == process_id) {
        entry.current_break = current_break;
        return;
    }

    table.push(ProcessBrkState {
        process_id,
        current_break,
    });
}

fn current_process_group_id() -> u64 {
    let process_id = current_process_id_value();
    let table = process_group_table_mut();
    if let Some(entry) = table.iter().find(|entry| entry.process_id == process_id) {
        return entry.process_group_id;
    }

    table.push(ProcessGroupState {
        process_id,
        process_group_id: process_id,
        session_id: process_id,
    });
    process_id
}

fn current_session_id() -> u64 {
    let process_id = current_process_id_value();
    let table = process_group_table_mut();
    if let Some(entry) = table.iter().find(|entry| entry.process_id == process_id) {
        return entry.session_id;
    }

    table.push(ProcessGroupState {
        process_id,
        process_group_id: process_id,
        session_id: process_id,
    });
    process_id
}

fn set_process_group_id(process_group_id: u64) {
    let process_id = current_process_id_value();
    let table = process_group_table_mut();
    if let Some(entry) = table.iter_mut().find(|entry| entry.process_id == process_id) {
        entry.process_group_id = process_group_id;
        return;
    }

    table.push(ProcessGroupState {
        process_id,
        process_group_id,
        session_id: process_id,
    });
}

fn set_session_and_group_id(session_id: u64, process_group_id: u64) {
    let process_id = current_process_id_value();
    let table = process_group_table_mut();
    if let Some(entry) = table.iter_mut().find(|entry| entry.process_id == process_id) {
        entry.session_id = session_id;
        entry.process_group_id = process_group_id;
        return;
    }

    table.push(ProcessGroupState {
        process_id,
        process_group_id,
        session_id,
    });
}

fn default_rlimit_for_resource(resource: u32) -> Option<LinuxRlimit64> {
    match resource {
        RLIMIT_CPU
        | RLIMIT_FSIZE
        | RLIMIT_DATA
        | RLIMIT_CORE
        | RLIMIT_RSS
        | RLIMIT_NPROC
        | RLIMIT_MEMLOCK
        | RLIMIT_AS
        | RLIMIT_LOCKS
        | RLIMIT_SIGPENDING
        | RLIMIT_MSGQUEUE
        | RLIMIT_NICE
        | RLIMIT_RTPRIO
        | RLIMIT_RTTIME => Some(LinuxRlimit64 {
            rlim_cur: RLIM_INFINITY,
            rlim_max: RLIM_INFINITY,
        }),
        RLIMIT_STACK => Some(LinuxRlimit64 {
            rlim_cur: 8 * 1024 * 1024,
            rlim_max: 8 * 1024 * 1024,
        }),
        RLIMIT_NOFILE => {
            let limit = MAX_OPEN_FILES as u64;
            Some(LinuxRlimit64 {
                rlim_cur: limit,
                rlim_max: limit,
            })
        }
        _ => None,
    }
}

fn current_process_rlimit(resource: u32) -> Option<LinuxRlimit64> {
    let default = default_rlimit_for_resource(resource)?;
    let process_id = current_process_id_value();
    let table = rlimit_table_mut();
    if let Some(entry) = table
        .iter()
        .find(|entry| entry.process_id == process_id && entry.resource == resource)
    {
        return Some(entry.limits);
    }

    table.push(ProcessRlimitState {
        process_id,
        resource,
        limits: default,
    });
    Some(default)
}

fn validate_rlimit_update(resource: u32, limit: LinuxRlimit64) -> Result<(), i64> {
    if limit.rlim_cur > limit.rlim_max {
        return Err(EINVAL);
    }
    if default_rlimit_for_resource(resource).is_none() {
        return Err(EINVAL);
    }
    if resource == RLIMIT_NOFILE {
        let upper_bound = MAX_OPEN_FILES as u64;
        if limit.rlim_cur > upper_bound || limit.rlim_max > upper_bound {
            return Err(EPERM);
        }
    }
    Ok(())
}

fn set_process_rlimit(resource: u32, limits: LinuxRlimit64) {
    let process_id = current_process_id_value();
    let table = rlimit_table_mut();
    if let Some(entry) = table
        .iter_mut()
        .find(|entry| entry.process_id == process_id && entry.resource == resource)
    {
        entry.limits = limits;
        return;
    }

    table.push(ProcessRlimitState {
        process_id,
        resource,
        limits,
    });
}

fn default_process_comm_name() -> [u8; TASK_COMM_LEN] {
    let mut name = [0u8; TASK_COMM_LEN];
    let thread_name = sched::stats().current_thread_name.as_bytes();
    let copy_len = min(thread_name.len(), TASK_COMM_LEN.saturating_sub(1));
    name[..copy_len].copy_from_slice(&thread_name[..copy_len]);
    name
}

fn current_process_prctl_state() -> ProcessPrctlState {
    let process_id = current_process_id_value();
    let table = prctl_table_mut();
    if let Some(entry) = table.iter().find(|entry| entry.process_id == process_id) {
        return ProcessPrctlState {
            process_id,
            name: entry.name,
            dumpable: entry.dumpable,
        };
    }

    let default = ProcessPrctlState {
        process_id,
        name: default_process_comm_name(),
        dumpable: 1,
    };
    table.push(ProcessPrctlState {
        process_id,
        name: default.name,
        dumpable: default.dumpable,
    });
    default
}

fn set_process_comm_name(name: [u8; TASK_COMM_LEN]) {
    let process_id = current_process_id_value();
    let table = prctl_table_mut();
    if let Some(entry) = table.iter_mut().find(|entry| entry.process_id == process_id) {
        entry.name = name;
        return;
    }

    table.push(ProcessPrctlState {
        process_id,
        name,
        dumpable: 1,
    });
}

fn current_process_comm_name() -> [u8; TASK_COMM_LEN] {
    current_process_prctl_state().name
}

fn set_process_dumpable(dumpable: i32) {
    let process_id = current_process_id_value();
    let table = prctl_table_mut();
    if let Some(entry) = table.iter_mut().find(|entry| entry.process_id == process_id) {
        entry.dumpable = dumpable;
        return;
    }

    table.push(ProcessPrctlState {
        process_id,
        name: default_process_comm_name(),
        dumpable,
    });
}

fn current_process_dumpable() -> i32 {
    current_process_prctl_state().dumpable
}

fn current_process_robust_list() -> (usize, usize) {
    let process_id = current_process_id_value();
    let table = robust_list_table_mut();
    if let Some(entry) = table.iter().find(|entry| entry.process_id == process_id) {
        return (entry.head, entry.len);
    }

    let default_len = size_of::<LinuxRobustListHead>();
    table.push(ProcessRobustListState {
        process_id,
        head: 0,
        len: default_len,
    });
    (0, default_len)
}

fn set_process_robust_list(head: usize, len: usize) {
    let process_id = current_process_id_value();
    let table = robust_list_table_mut();
    if let Some(entry) = table.iter_mut().find(|entry| entry.process_id == process_id) {
        entry.head = head;
        entry.len = len;
        return;
    }

    table.push(ProcessRobustListState { process_id, head, len });
}

fn current_process_rseq_state() -> ProcessRseqState {
    let process_id = current_process_id_value();
    let table = rseq_table_mut();
    if let Some(entry) = table.iter().find(|entry| entry.process_id == process_id) {
        return ProcessRseqState {
            process_id,
            address: entry.address,
            length: entry.length,
            signature: entry.signature,
            registered: entry.registered,
        };
    }

    let default = ProcessRseqState {
        process_id,
        address: 0,
        length: size_of::<LinuxRseqArea>() as u32,
        signature: RSEQ_SIGNATURE,
        registered: false,
    };
    table.push(ProcessRseqState {
        process_id,
        address: default.address,
        length: default.length,
        signature: default.signature,
        registered: default.registered,
    });
    default
}

fn set_process_rseq_state(state: ProcessRseqState) {
    let process_id = current_process_id_value();
    let table = rseq_table_mut();
    if let Some(entry) = table.iter_mut().find(|entry| entry.process_id == process_id) {
        entry.address = state.address;
        entry.length = state.length;
        entry.signature = state.signature;
        entry.registered = state.registered;
        return;
    }

    table.push(ProcessRseqState {
        process_id,
        address: state.address,
        length: state.length,
        signature: state.signature,
        registered: state.registered,
    });
}

fn clear_process_rseq_state() {
    let process_id = current_process_id_value();
    let table = rseq_table_mut();
    if let Some(entry) = table.iter_mut().find(|entry| entry.process_id == process_id) {
        entry.address = 0;
        entry.length = size_of::<LinuxRseqArea>() as u32;
        entry.signature = RSEQ_SIGNATURE;
        entry.registered = false;
        return;
    }

    table.push(ProcessRseqState {
        process_id,
        address: 0,
        length: size_of::<LinuxRseqArea>() as u32,
        signature: RSEQ_SIGNATURE,
        registered: false,
    });
}

fn current_process_signal_mask() -> u64 {
    let process_id = current_process_id_value();
    let table = signal_mask_table_mut();
    if let Some(entry) = table.iter().find(|entry| entry.process_id == process_id) {
        return entry.mask;
    }

    table.push(ProcessSignalMask {
        process_id,
        mask: 0,
    });
    0
}

fn set_process_signal_mask(mask: u64) {
    let process_id = current_process_id_value();
    let table = signal_mask_table_mut();
    if let Some(entry) = table.iter_mut().find(|entry| entry.process_id == process_id) {
        entry.mask = mask;
        return;
    }

    table.push(ProcessSignalMask { process_id, mask });
}

fn current_signal_action(signum: u8) -> LinuxKernelSigAction {
    let process_id = current_process_id_value();
    let table = signal_action_table_mut();
    table
        .iter()
        .find(|entry| entry.process_id == process_id && entry.signum == signum)
        .map(|entry| entry.action)
        .unwrap_or_else(LinuxKernelSigAction::empty)
}

fn set_signal_action(signum: u8, action: LinuxKernelSigAction) {
    let process_id = current_process_id_value();
    let table = signal_action_table_mut();
    if let Some(entry) = table
        .iter_mut()
        .find(|entry| entry.process_id == process_id && entry.signum == signum)
    {
        entry.action = action;
        return;
    }

    table.push(ProcessSignalAction {
        process_id,
        signum,
        action,
    });
}

fn map_anonymous_region(length: usize, prot: u64) -> Result<i64, i64> {
    let length = align_up_to_page(length)?;
    let layout = Layout::from_size_align(length, MMAP_PAGE_SIZE).map_err(|_| EINVAL)?;
    let ptr = unsafe { alloc_zeroed(layout) };
    if ptr.is_null() {
        return Err(ENOMEM);
    }

    let process_id = current_process_id_value();
    let address = ptr as usize;
    let address_i64 = match i64::try_from(address) {
        Ok(value) => value,
        Err(_) => {
            unsafe { dealloc(ptr, layout) };
            return Err(ERANGE);
        }
    };

    let table = mapping_table_mut();
    table.push(ProcessMapping {
        process_id,
        base: address,
        len: length,
        prot: prot & PROT_MASK,
    });
    Ok(address_i64)
}

fn unmap_region(address: usize, length: usize) -> Result<i64, i64> {
    let length = align_up_to_page(length)?;
    let process_id = current_process_id_value();
    let table = mapping_table_mut();
    let index = table
        .iter()
        .position(|mapping| mapping.process_id == process_id && mapping.base == address && mapping.len == length)
        .ok_or(EINVAL)?;
    let mapping = table.remove(index);
    free_mapping(mapping)?;
    Ok(0)
}

fn protect_region(address: usize, length: usize, prot: u64) -> Result<i64, i64> {
    let length = align_up_to_page(length)?;
    let process_id = current_process_id_value();
    let table = mapping_table_mut();
    let mapping = table
        .iter_mut()
        .find(|mapping| mapping.process_id == process_id && mapping.base == address && mapping.len == length)
        .ok_or(EINVAL)?;
    mapping.prot = prot & PROT_MASK;
    Ok(0)
}

fn align_up_to_page(value: usize) -> Result<usize, i64> {
    if value == 0 {
        return Err(EINVAL);
    }
    value
        .checked_add(MMAP_PAGE_SIZE.saturating_sub(1))
        .map(|value| value & !(MMAP_PAGE_SIZE - 1))
        .ok_or(ERANGE)
}

fn to_address_outcome(address: usize) -> SyscallOutcome {
    match i64::try_from(address) {
        Ok(value) => SyscallOutcome::success(value),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn free_mapping(mapping: ProcessMapping) -> Result<(), i64> {
    let layout = Layout::from_size_align(mapping.len, MMAP_PAGE_SIZE).map_err(|_| EINVAL)?;
    unsafe { dealloc(mapping.base as *mut u8, layout) };
    Ok(())
}

#[derive(Copy, Clone)]
enum DuplicateTarget {
    LowestAvailable,
    Exact,
}

fn duplicate_fd(
    source_fd: i32,
    target_or_minimum_fd: i32,
    target: DuplicateTarget,
    fd_flags: u32,
) -> Result<i64, i64> {
    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    if !table
        .files
        .iter()
        .any(|file| file.owner_process_id == owner_process_id && file.fd == source_fd)
    {
        return Err(EBADF);
    }

    let destination_fd = match target {
        DuplicateTarget::LowestAvailable => {
            find_available_fd_for_process(table, owner_process_id, target_or_minimum_fd).ok_or(EMFILE)?
        }
        DuplicateTarget::Exact => target_or_minimum_fd,
    };
    let replaced_index = if matches!(target, DuplicateTarget::Exact) {
        table
            .files
            .iter()
            .position(|file| file.owner_process_id == owner_process_id && file.fd == destination_fd)
    } else {
        None
    };
    if table.files.len() >= MAX_OPEN_FILES && replaced_index.is_none() {
        return Err(EMFILE);
    }
    if let Some(index) = replaced_index {
        table.files.remove(index);
    }

    let source = table
        .files
        .iter()
        .find(|file| file.owner_process_id == owner_process_id && file.fd == source_fd)
        .ok_or(EBADF)?;
    let duplicate = duplicate_file_descriptor(source, destination_fd, fd_flags & FD_CLOEXEC);
    table.files.push(duplicate);

    i64::try_from(destination_fd).map_err(|_| ERANGE)
}

fn find_available_fd_for_process(table: &FdTable, owner_process_id: u64, minimum_fd: i32) -> Option<i32> {
    let mut candidate = minimum_fd.max(3);
    loop {
        if !table
            .files
            .iter()
            .any(|file| file.owner_process_id == owner_process_id && file.fd == candidate)
        {
            return Some(candidate);
        }
        if candidate == i32::MAX {
            return None;
        }
        candidate = candidate.saturating_add(1);
    }
}

fn duplicate_file_descriptor(source: &OpenFile, destination_fd: i32, fd_flags: u32) -> OpenFile {
    OpenFile {
        fd: destination_fd,
        fd_flags,
        owner_process_id: source.owner_process_id,
        mount: source.mount,
        kind: source.kind,
        executable: source.executable,
        path: source.path.clone(),
        offset: source.offset,
        content: source.content.clone(),
    }
}

fn get_descriptor_flags(fd: i32) -> Result<u32, i64> {
    if fd == 0 || fd as u64 == STDOUT_FD || fd as u64 == STDERR_FD {
        return Ok(0);
    }

    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    let open = table
        .files
        .iter()
        .find(|file| file.owner_process_id == owner_process_id && file.fd == fd)
        .ok_or(EBADF)?;
    Ok(open.fd_flags)
}

fn set_descriptor_flags(fd: i32, flags: u32) -> Result<i64, i64> {
    if fd == 0 || fd as u64 == STDOUT_FD || fd as u64 == STDERR_FD {
        return Ok(0);
    }

    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    let open = table
        .files
        .iter_mut()
        .find(|file| file.owner_process_id == owner_process_id && file.fd == fd)
        .ok_or(EBADF)?;
    open.fd_flags = flags & FD_CLOEXEC;
    Ok(0)
}

fn get_status_flags(fd: i32) -> Result<i64, i64> {
    if fd == 0 || fd as u64 == STDOUT_FD || fd as u64 == STDERR_FD {
        return i64::try_from(O_RDONLY).map_err(|_| ERANGE);
    }

    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    let open = table
        .files
        .iter()
        .find(|file| file.owner_process_id == owner_process_id && file.fd == fd)
        .ok_or(EBADF)?;
    let mut flags = O_RDONLY;
    if open.kind == VfsNodeKind::Directory {
        flags |= O_DIRECTORY;
    }

    i64::try_from(flags).map_err(|_| ERANGE)
}

fn open_file_path_and_kind_for_process(fd: i32) -> Result<(String, VfsNodeKind), i64> {
    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    let open = table
        .files
        .iter()
        .find(|file| file.owner_process_id == owner_process_id && file.fd == fd)
        .ok_or(EBADF)?;
    Ok((open.path.clone(), open.kind))
}

fn alloc_open_file(
    path: String,
    mount: VfsMountKind,
    kind: VfsNodeKind,
    executable: bool,
    content: Vec<u8>,
) -> Result<i64, i64> {
    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    if table.files.len() >= MAX_OPEN_FILES {
        return Err(EMFILE);
    }

    let fd = table.next_fd;
    table.next_fd = table.next_fd.checked_add(1).ok_or(ERANGE)?;
    table.files.push(OpenFile {
        fd,
        fd_flags: 0,
        owner_process_id,
        mount,
        kind,
        executable,
        path,
        offset: 0,
        content,
    });
    Ok(fd as i64)
}

fn read_open_file(fd: i32, destination_ptr: usize, count: usize) -> Result<i64, i64> {
    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    let open = table
        .files
        .iter_mut()
        .find(|file| file.fd == fd && file.owner_process_id == owner_process_id)
        .ok_or(EBADF)?;
    if open.kind == VfsNodeKind::Directory {
        return Err(EISDIR);
    }
    let _ = &open.path;

    if count == 0 {
        return Ok(0);
    }

    let available = open.content.len().saturating_sub(open.offset);
    let read_len = min(count, available);
    let bytes = &open.content[open.offset..open.offset + read_len];
    uaccess::copyout(bytes, destination_ptr).map_err(map_uaccess_error)?;
    open.offset = open.offset.saturating_add(read_len);

    i64::try_from(read_len).map_err(|_| ERANGE)
}

fn read_open_file_at_offset(fd: i32, destination_ptr: usize, count: usize, offset: i64) -> Result<i64, i64> {
    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    let open = table
        .files
        .iter()
        .find(|file| file.fd == fd && file.owner_process_id == owner_process_id)
        .ok_or(EBADF)?;
    if open.kind == VfsNodeKind::Directory {
        return Err(EISDIR);
    }
    let start = usize::try_from(offset).map_err(|_| ERANGE)?;
    if start >= open.content.len() {
        return Ok(0);
    }

    let available = open.content.len().saturating_sub(start);
    let read_len = min(count, available);
    let bytes = &open.content[start..start + read_len];
    uaccess::copyout(bytes, destination_ptr).map_err(map_uaccess_error)?;

    i64::try_from(read_len).map_err(|_| ERANGE)
}

fn close_open_file(fd: i32) -> Result<i64, i64> {
    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    if let Some(position) = table
        .files
        .iter()
        .position(|file| file.fd == fd && file.owner_process_id == owner_process_id)
    {
        table.files.remove(position);
        return Ok(0);
    }
    Err(EBADF)
}

fn stat_open_file(fd: i32, stat_ptr: usize) -> Result<i64, i64> {
    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    let open = table
        .files
        .iter()
        .find(|file| file.fd == fd && file.owner_process_id == owner_process_id)
        .ok_or(EBADF)?;

    let stat = build_linux_stat(open.mount, open.kind, open.executable, open.content.len(), &open.path)?;
    copyout_struct(stat_ptr, &stat)?;
    Ok(0)
}

fn read_directory_entries(fd: i32, destination_ptr: usize, count: usize) -> Result<i64, i64> {
    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    let open = table
        .files
        .iter_mut()
        .find(|file| file.fd == fd && file.owner_process_id == owner_process_id)
        .ok_or(EBADF)?;
    if open.kind != VfsNodeKind::Directory {
        return Err(ENOTDIR);
    }

    let entries = directory_entry_names(&open.content)?;
    if open.offset >= entries.len() {
        return Ok(0);
    }

    let mut buffer = vec![0u8; count];
    let mut cursor = 0usize;
    let mut index = open.offset;
    while index < entries.len() {
        let name = entries[index];
        let child_path = child_path_for_entry(&open.path, name);
        let (ino, d_type) = match vfs::lookup(&child_path) {
            Some(node) => (hash_path_to_ino(node.mount, &node.path), dirent_type_from_node(node.kind)),
            None => (hash_path_to_ino(open.mount, &child_path), DT_UNKNOWN),
        };
        let next_offset = i64::try_from(index.saturating_add(1)).map_err(|_| ERANGE)?;
        let Some(next_cursor) =
            append_linux_dirent64(&mut buffer, cursor, ino, next_offset, d_type, name)
        else {
            break;
        };
        cursor = next_cursor;
        index = index.saturating_add(1);
    }

    if cursor == 0 {
        return Err(EINVAL);
    }

    uaccess::copyout(&buffer[..cursor], destination_ptr).map_err(map_uaccess_error)?;
    open.offset = index;
    i64::try_from(cursor).map_err(|_| ERANGE)
}

fn seek_open_file(fd: i32, offset: i64, whence: i32) -> Result<i64, i64> {
    let owner_process_id = current_process_id_value();
    let table = fd_table_mut();
    let open = table
        .files
        .iter_mut()
        .find(|file| file.fd == fd && file.owner_process_id == owner_process_id)
        .ok_or(EBADF)?;

    let end = if open.kind == VfsNodeKind::Directory {
        directory_entry_count(&open.content)?
    } else {
        open.content.len()
    };

    let base = match whence {
        SEEK_SET => 0,
        SEEK_CUR => i64::try_from(open.offset).map_err(|_| ERANGE)?,
        SEEK_END => i64::try_from(end).map_err(|_| ERANGE)?,
        _ => return Err(EINVAL),
    };

    let next_offset = base.checked_add(offset).ok_or(ERANGE)?;
    if next_offset < 0 {
        return Err(EINVAL);
    }
    open.offset = usize::try_from(next_offset).map_err(|_| ERANGE)?;
    i64::try_from(open.offset).map_err(|_| ERANGE)
}

fn purge_open_files_for_process(process_id: u64) {
    let table = fd_table_mut();
    table.files.retain(|file| file.owner_process_id != process_id);
}

fn purge_working_directory_for_process(process_id: u64) {
    let table = cwd_table_mut();
    table.retain(|entry| entry.process_id != process_id);
}

fn purge_process_umask(process_id: u64) {
    let table = umask_table_mut();
    table.retain(|entry| entry.process_id != process_id);
}

fn purge_process_clear_tid_address(process_id: u64) {
    let table = clear_tid_table_mut();
    table.retain(|entry| entry.process_id != process_id);
}

fn purge_process_mappings(process_id: u64) {
    let table = mapping_table_mut();
    let mut index = 0usize;
    while index < table.len() {
        if table[index].process_id != process_id {
            index = index.saturating_add(1);
            continue;
        }
        let mapping = table.remove(index);
        let _ = free_mapping(mapping);
    }
}

fn purge_process_brk(process_id: u64) {
    let table = brk_table_mut();
    table.retain(|entry| entry.process_id != process_id);
}

fn purge_process_group_state(process_id: u64) {
    let table = process_group_table_mut();
    table.retain(|entry| entry.process_id != process_id);
}

fn purge_process_rlimits(process_id: u64) {
    let table = rlimit_table_mut();
    table.retain(|entry| entry.process_id != process_id);
}

fn purge_process_prctl_state(process_id: u64) {
    let table = prctl_table_mut();
    table.retain(|entry| entry.process_id != process_id);
}

fn purge_process_robust_list_state(process_id: u64) {
    let table = robust_list_table_mut();
    table.retain(|entry| entry.process_id != process_id);
}

fn purge_process_rseq_state(process_id: u64) {
    let table = rseq_table_mut();
    table.retain(|entry| entry.process_id != process_id);
}

fn purge_process_signal_mask(process_id: u64) {
    let table = signal_mask_table_mut();
    table.retain(|entry| entry.process_id != process_id);
}

fn purge_process_signal_actions(process_id: u64) {
    let table = signal_action_table_mut();
    table.retain(|entry| entry.process_id != process_id);
}

fn fd_table_mut() -> &'static mut FdTable {
    let slot = unsafe { &mut *FD_TABLE.get() };
    if slot.is_none() {
        *slot = Some(FdTable::new());
    }
    slot.as_mut().expect("fd table initialized")
}

fn cwd_table_mut() -> &'static mut Vec<ProcessCwd> {
    let slot = unsafe { &mut *CWD_TABLE.get() };
    if slot.is_none() {
        *slot = Some(Vec::new());
    }
    slot.as_mut().expect("cwd table initialized")
}

fn umask_table_mut() -> &'static mut Vec<ProcessUmask> {
    let slot = unsafe { &mut *UMASK_TABLE.get() };
    if slot.is_none() {
        *slot = Some(Vec::new());
    }
    slot.as_mut().expect("umask table initialized")
}

fn clear_tid_table_mut() -> &'static mut Vec<ProcessClearTidAddress> {
    let slot = unsafe { &mut *CLEAR_TID_TABLE.get() };
    if slot.is_none() {
        *slot = Some(Vec::new());
    }
    slot.as_mut().expect("clear-tid table initialized")
}

fn mapping_table_mut() -> &'static mut Vec<ProcessMapping> {
    let slot = unsafe { &mut *MAPPING_TABLE.get() };
    if slot.is_none() {
        *slot = Some(Vec::new());
    }
    slot.as_mut().expect("mapping table initialized")
}

fn brk_table_mut() -> &'static mut Vec<ProcessBrkState> {
    let slot = unsafe { &mut *BRK_TABLE.get() };
    if slot.is_none() {
        *slot = Some(Vec::new());
    }
    slot.as_mut().expect("brk table initialized")
}

fn process_group_table_mut() -> &'static mut Vec<ProcessGroupState> {
    let slot = unsafe { &mut *PROCESS_GROUP_TABLE.get() };
    if slot.is_none() {
        *slot = Some(Vec::new());
    }
    slot.as_mut().expect("process group table initialized")
}

fn rlimit_table_mut() -> &'static mut Vec<ProcessRlimitState> {
    let slot = unsafe { &mut *RLIMIT_TABLE.get() };
    if slot.is_none() {
        *slot = Some(Vec::new());
    }
    slot.as_mut().expect("rlimit table initialized")
}

fn prctl_table_mut() -> &'static mut Vec<ProcessPrctlState> {
    let slot = unsafe { &mut *PRCTL_TABLE.get() };
    if slot.is_none() {
        *slot = Some(Vec::new());
    }
    slot.as_mut().expect("prctl table initialized")
}

fn robust_list_table_mut() -> &'static mut Vec<ProcessRobustListState> {
    let slot = unsafe { &mut *ROBUST_LIST_TABLE.get() };
    if slot.is_none() {
        *slot = Some(Vec::new());
    }
    slot.as_mut().expect("robust-list table initialized")
}

fn rseq_table_mut() -> &'static mut Vec<ProcessRseqState> {
    let slot = unsafe { &mut *RSEQ_TABLE.get() };
    if slot.is_none() {
        *slot = Some(Vec::new());
    }
    slot.as_mut().expect("rseq table initialized")
}

fn signal_mask_table_mut() -> &'static mut Vec<ProcessSignalMask> {
    let slot = unsafe { &mut *SIGNAL_MASK_TABLE.get() };
    if slot.is_none() {
        *slot = Some(Vec::new());
    }
    slot.as_mut().expect("signal mask table initialized")
}

fn signal_action_table_mut() -> &'static mut Vec<ProcessSignalAction> {
    let slot = unsafe { &mut *SIGNAL_ACTION_TABLE.get() };
    if slot.is_none() {
        *slot = Some(Vec::new());
    }
    slot.as_mut().expect("signal action table initialized")
}

fn copyin_c_string(ptr: usize, max_len: usize) -> Result<String, i64> {
    let mut bytes = Vec::new();
    for index in 0..max_len {
        let address = ptr.checked_add(index).ok_or(ERANGE)?;
        let mut byte = [0u8; 1];
        uaccess::copyin(address, &mut byte).map_err(map_uaccess_error)?;
        if byte[0] == 0 {
            let text = str::from_utf8(&bytes).map_err(|_| EINVAL)?;
            return Ok(String::from(text));
        }
        bytes.push(byte[0]);
    }

    Err(ERANGE)
}

fn copyin_bytes(ptr: usize, len: usize) -> Result<Vec<u8>, i64> {
    let mut bytes = vec![0u8; len];
    uaccess::copyin(ptr, &mut bytes).map_err(map_uaccess_error)?;
    Ok(bytes)
}

fn copyin_sigset(ptr: usize, sigset_size: usize) -> Result<u64, i64> {
    let bytes = copyin_bytes(ptr, sigset_size)?;
    let mut value = 0u64;
    for (index, byte) in bytes.iter().copied().enumerate().take(RT_SIGSET_SIZE) {
        value |= u64::from(byte) << (index * 8);
    }
    Ok(value)
}

fn copyin_sigaction(ptr: usize) -> Result<LinuxKernelSigAction, i64> {
    let bytes = copyin_bytes(ptr, size_of::<LinuxKernelSigAction>())?;
    if bytes.len() != size_of::<LinuxKernelSigAction>() {
        return Err(EINVAL);
    }

    let handler = u64::from_le_bytes(bytes[0..8].try_into().map_err(|_| EINVAL)?);
    let flags = u64::from_le_bytes(bytes[8..16].try_into().map_err(|_| EINVAL)?);
    let restorer = u64::from_le_bytes(bytes[16..24].try_into().map_err(|_| EINVAL)?);
    let mask = u64::from_le_bytes(bytes[24..32].try_into().map_err(|_| EINVAL)?);
    Ok(LinuxKernelSigAction {
        handler,
        flags,
        restorer,
        mask,
    })
}

fn copyin_rlimit(ptr: usize) -> Result<LinuxRlimit64, i64> {
    let bytes = copyin_bytes(ptr, size_of::<LinuxRlimit64>())?;
    if bytes.len() != size_of::<LinuxRlimit64>() {
        return Err(EINVAL);
    }

    let rlim_cur = u64::from_le_bytes(bytes[0..8].try_into().map_err(|_| EINVAL)?);
    let rlim_max = u64::from_le_bytes(bytes[8..16].try_into().map_err(|_| EINVAL)?);
    Ok(LinuxRlimit64 { rlim_cur, rlim_max })
}

fn copyin_comm_name(ptr: usize) -> Result<[u8; TASK_COMM_LEN], i64> {
    let bytes = copyin_bytes(ptr, TASK_COMM_LEN)?;
    let mut name = [0u8; TASK_COMM_LEN];
    let mut copy_len = bytes.iter().position(|&byte| byte == 0).unwrap_or(bytes.len());
    copy_len = min(copy_len, TASK_COMM_LEN.saturating_sub(1));
    name[..copy_len].copy_from_slice(&bytes[..copy_len]);
    name[copy_len] = 0;
    Ok(name)
}

fn copyin_iovec_at(iov_ptr: usize, index: usize) -> Result<LinuxIovec, i64> {
    let record_size = size_of::<LinuxIovec>();
    let offset = index.checked_mul(record_size).ok_or(ERANGE)?;
    let address = iov_ptr.checked_add(offset).ok_or(ERANGE)?;
    let bytes = copyin_bytes(address, record_size)?;
    if bytes.len() != record_size {
        return Err(EINVAL);
    }

    let iov_base = u64::from_le_bytes(bytes[0..8].try_into().map_err(|_| EINVAL)?);
    let iov_len = u64::from_le_bytes(bytes[8..16].try_into().map_err(|_| EINVAL)?);
    Ok(LinuxIovec { iov_base, iov_len })
}

fn copyout_struct<T: Copy>(ptr: usize, value: &T) -> Result<(), i64> {
    let bytes = unsafe { slice::from_raw_parts((value as *const T).cast::<u8>(), size_of_val(value)) };
    uaccess::copyout(bytes, ptr).map_err(map_uaccess_error)
}

fn map_uaccess_error(error: UserCopyError) -> i64 {
    error.as_errno()
}

fn fill_pseudo_random_bytes(destination: &mut [u8]) {
    let stats = sched::stats();
    let mut state = time::uptime_nanoseconds()
        ^ stats.current_process_id.rotate_left(13)
        ^ stats.current_thread_id.rotate_left(29)
        ^ 0x9e37_79b9_7f4a_7c15;
    if state == 0 {
        state = 0xa076_1d64_78bd_642f;
    }

    for byte in destination.iter_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = (state & 0xff) as u8;
    }
}

fn sample_random_u64(bytes: &[u8]) -> u64 {
    let mut prefix = [0u8; 8];
    let copy_len = min(prefix.len(), bytes.len());
    prefix[..copy_len].copy_from_slice(&bytes[..copy_len]);
    u64::from_le_bytes(prefix)
}

fn write_uts_field(field: &mut [u8; 65], value: &str) {
    let bytes = value.as_bytes();
    let copy_len = min(bytes.len(), field.len().saturating_sub(1));
    field[..copy_len].copy_from_slice(&bytes[..copy_len]);
    field[copy_len] = 0;
}

fn copy_c_field_prefix(output: &mut [u8], field: &[u8; 65]) -> usize {
    let mut length = 0usize;
    while length < output.len() && length < field.len() {
        let byte = field[length];
        if byte == 0 {
            break;
        }
        output[length] = byte;
        length += 1;
    }
    length
}

fn machine_str(machine_bytes: &[u8], machine_len: usize) -> &str {
    match str::from_utf8(&machine_bytes[..machine_len]) {
        Ok(machine) => machine,
        Err(_) => "<invalid>",
    }
}
