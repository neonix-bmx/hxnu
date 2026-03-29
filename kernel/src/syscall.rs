use alloc::string::String;
use core::cmp::min;
use core::mem::size_of;
use core::slice;
use core::str;

use crate::sched;
use crate::time;
use crate::tty;

pub const LINUX_ABI_NAME: &str = "linux-x86_64-bootstrap";
pub const GHOST_ABI_NAME: &str = "ghost-bootstrap";
pub const HXNU_ABI_NAME: &str = "hxnu-native-bootstrap";

pub const LINUX_SYS_WRITE: u64 = 1;
pub const LINUX_SYS_SCHED_YIELD: u64 = 24;
pub const LINUX_SYS_GETPID: u64 = 39;
pub const LINUX_SYS_EXIT: u64 = 60;
pub const LINUX_SYS_UNAME: u64 = 63;
pub const LINUX_SYS_GETPPID: u64 = 110;
pub const LINUX_SYS_GETTID: u64 = 186;
pub const LINUX_SYS_CLOCK_GETTIME: u64 = 228;
pub const LINUX_SYS_EXIT_GROUP: u64 = 231;

pub const GHOST_SYS_WRITE: u64 = 1;
pub const GHOST_SYS_YIELD: u64 = 2;
pub const GHOST_SYS_GETPID: u64 = 3;
pub const GHOST_SYS_GETTID: u64 = 4;
pub const GHOST_SYS_UPTIME_NSEC: u64 = 5;
pub const GHOST_SYS_UNAME: u64 = 6;
pub const GHOST_SYS_EXIT_GROUP: u64 = 7;

pub const HXNU_SYS_LOG_WRITE: u64 = 0x484e_0001;
pub const HXNU_SYS_THREAD_SELF: u64 = 0x484e_0002;
pub const HXNU_SYS_PROCESS_SELF: u64 = 0x484e_0003;
pub const HXNU_SYS_UPTIME_NSEC: u64 = 0x484e_0004;
pub const HXNU_SYS_SCHED_YIELD: u64 = 0x484e_0005;
pub const HXNU_SYS_ABI_VERSION: u64 = 0x484e_0006;
pub const HXNU_SYS_EXIT_GROUP: u64 = 0x484e_00ff;

const HXNU_NATIVE_ABI_VERSION: i64 = 0x0001_0000;

const LINUX_CLOCK_REALTIME: i32 = 0;
const LINUX_CLOCK_MONOTONIC: i32 = 1;

const MAX_WRITE_BYTES: usize = 16 * 1024;

const EBADF: i64 = 9;
const EFAULT: i64 = 14;
const EINVAL: i64 = 22;
const ENOSYS: i64 = 38;
const ERANGE: i64 = 34;

const STDOUT_FD: u64 = 1;
const STDERR_FD: u64 = 2;

const LOW_CANONICAL_MAX: usize = (1usize << 47) - 1;
const HIGH_CANONICAL_MIN: usize = !LOW_CANONICAL_MAX;

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
    pub getpid_result: i64,
    pub getppid_result: i64,
    pub gettid_result: i64,
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
    pub getpid_result: i64,
    pub gettid_result: i64,
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
    pub process_self_result: i64,
    pub thread_self_result: i64,
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

pub fn dispatch(abi: SyscallAbi, number: u64, args: [u64; 6]) -> SyscallOutcome {
    match abi {
        SyscallAbi::LinuxBootstrap => dispatch_linux_bootstrap(number, args),
        SyscallAbi::GhostBootstrap => dispatch_ghost_bootstrap(number, args),
        SyscallAbi::HxnuNativeBootstrap => dispatch_hxnu_bootstrap(number, args),
    }
}

pub fn dispatch_linux_bootstrap(number: u64, args: [u64; 6]) -> SyscallOutcome {
    match number {
        LINUX_SYS_WRITE => write_with_fd(args),
        LINUX_SYS_SCHED_YIELD => SyscallOutcome::success(0),
        LINUX_SYS_GETPID => process_id(),
        LINUX_SYS_GETPPID => SyscallOutcome::success(0),
        LINUX_SYS_GETTID => thread_id(),
        LINUX_SYS_CLOCK_GETTIME => linux_clock_gettime(args),
        LINUX_SYS_UNAME => linux_uname(args),
        LINUX_SYS_EXIT | LINUX_SYS_EXIT_GROUP => exit_group(args),
        _ => SyscallOutcome::errno(ENOSYS),
    }
}

pub fn dispatch_ghost_bootstrap(number: u64, args: [u64; 6]) -> SyscallOutcome {
    match number {
        GHOST_SYS_WRITE => write_with_fd(args),
        GHOST_SYS_YIELD => SyscallOutcome::success(0),
        GHOST_SYS_GETPID => process_id(),
        GHOST_SYS_GETTID => thread_id(),
        GHOST_SYS_UPTIME_NSEC => uptime_ns(),
        GHOST_SYS_UNAME => ghost_uname(args),
        GHOST_SYS_EXIT_GROUP => exit_group(args),
        _ => SyscallOutcome::errno(ENOSYS),
    }
}

pub fn dispatch_hxnu_bootstrap(number: u64, args: [u64; 6]) -> SyscallOutcome {
    match number {
        HXNU_SYS_LOG_WRITE => write_without_fd(args),
        HXNU_SYS_THREAD_SELF => thread_id(),
        HXNU_SYS_PROCESS_SELF => process_id(),
        HXNU_SYS_UPTIME_NSEC => uptime_ns(),
        HXNU_SYS_SCHED_YIELD => SyscallOutcome::success(0),
        HXNU_SYS_ABI_VERSION => SyscallOutcome::success(HXNU_NATIVE_ABI_VERSION),
        HXNU_SYS_EXIT_GROUP => exit_group(args),
        _ => SyscallOutcome::errno(ENOSYS),
    }
}

pub fn run_linux_bootstrap_probe() -> LinuxBootstrapProbe {
    static WRITE_SMOKE: &[u8] = b"HXNU: linux syscall write() compatibility smoke\n";
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

    let getpid_result = dispatch(abi, LINUX_SYS_GETPID, [0; 6]).value;
    let getppid_result = dispatch(abi, LINUX_SYS_GETPPID, [0; 6]).value;
    let gettid_result = dispatch(abi, LINUX_SYS_GETTID, [0; 6]).value;
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
        getpid_result,
        getppid_result,
        gettid_result,
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
    let getpid_result = dispatch(abi, GHOST_SYS_GETPID, [0; 6]).value;
    let gettid_result = dispatch(abi, GHOST_SYS_GETTID, [0; 6]).value;
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
        getpid_result,
        gettid_result,
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
    let abi = SyscallAbi::HxnuNativeBootstrap;

    let write_result = dispatch(
        abi,
        HXNU_SYS_LOG_WRITE,
        [WRITE_SMOKE.as_ptr() as u64, WRITE_SMOKE.len() as u64, 0, 0, 0, 0],
    )
    .value;
    let process_self_result = dispatch(abi, HXNU_SYS_PROCESS_SELF, [0; 6]).value;
    let thread_self_result = dispatch(abi, HXNU_SYS_THREAD_SELF, [0; 6]).value;
    let sched_yield_result = dispatch(abi, HXNU_SYS_SCHED_YIELD, [0; 6]).value;
    let uptime_result = dispatch(abi, HXNU_SYS_UPTIME_NSEC, [0; 6]).value;
    let abi_version_result = dispatch(abi, HXNU_SYS_ABI_VERSION, [0; 6]).value;

    let exit_group = dispatch(abi, HXNU_SYS_EXIT_GROUP, [23, 0, 0, 0, 0, 0]);
    let (exit_group_captured, exit_group_status) = exit_status(exit_group);

    HxnuBootstrapProbe {
        write_result,
        process_self_result,
        thread_self_result,
        sched_yield_result,
        uptime_result,
        abi_version_result,
        exit_group_captured,
        exit_group_status,
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

    let bytes = match trusted_const_slice(ptr, count) {
        Ok(bytes) => bytes,
        Err(error) => return SyscallOutcome::errno(error),
    };

    let text = sanitize_for_console(bytes);
    tty::write_str(&text);

    match i64::try_from(count) {
        Ok(written) => SyscallOutcome::success(written),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
}

fn process_id() -> SyscallOutcome {
    SyscallOutcome::success(1)
}

fn thread_id() -> SyscallOutcome {
    let tid = sched::stats().current_thread_id;
    match i64::try_from(tid) {
        Ok(tid) => SyscallOutcome::success(tid),
        Err(_) => SyscallOutcome::errno(ERANGE),
    }
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
    let destination = match trusted_mut_ptr::<LinuxTimespec>(ptr) {
        Ok(destination) => destination,
        Err(error) => return SyscallOutcome::errno(error),
    };

    let uptime_ns = time::uptime_nanoseconds();
    let timespec = LinuxTimespec {
        tv_sec: (uptime_ns / 1_000_000_000) as i64,
        tv_nsec: (uptime_ns % 1_000_000_000) as i64,
    };
    unsafe {
        destination.write(timespec);
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
    let destination = match trusted_mut_ptr::<LinuxUtsName>(ptr) {
        Ok(destination) => destination,
        Err(error) => return SyscallOutcome::errno(error),
    };

    let mut uts = LinuxUtsName::new();
    write_uts_field(&mut uts.sysname, sysname);
    write_uts_field(&mut uts.nodename, nodename);
    write_uts_field(&mut uts.release, release);
    write_uts_field(&mut uts.version, version);
    write_uts_field(&mut uts.machine, machine);
    write_uts_field(&mut uts.domainname, domainname);
    unsafe {
        destination.write(uts);
    }

    SyscallOutcome::success(0)
}

fn exit_group(args: [u64; 6]) -> SyscallOutcome {
    let status = args[0] as i32;
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

fn trusted_const_slice(ptr: usize, len: usize) -> Result<&'static [u8], i64> {
    validate_address_range(ptr, len)?;
    let source = ptr as *const u8;
    // Bootstrap mode: pointers are expected to reference kernel-mapped buffers.
    Ok(unsafe { slice::from_raw_parts(source, len) })
}

fn trusted_mut_ptr<T>(ptr: usize) -> Result<*mut T, i64> {
    validate_address_range(ptr, size_of::<T>())?;
    Ok(ptr as *mut T)
}

fn validate_address_range(start: usize, len: usize) -> Result<(), i64> {
    if len == 0 {
        return Ok(());
    }
    if start == 0 {
        return Err(EFAULT);
    }

    let end = start.checked_add(len - 1).ok_or(EFAULT)?;
    if !is_canonical_address(start) || !is_canonical_address(end) {
        return Err(EFAULT);
    }

    Ok(())
}

const fn is_canonical_address(address: usize) -> bool {
    address <= LOW_CANONICAL_MAX || address >= HIGH_CANONICAL_MIN
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
