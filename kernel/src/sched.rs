use core::arch::asm;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::arch;
use crate::time;

const BOOTSTRAP_TARGET_TICKS: u64 = 3;
const BOOTSTRAP_TIMEOUT_NS: u64 = 500_000_000;
const MAX_THREADS: usize = 8;
const KERNEL_STACK_SIZE: usize = 16 * 1024;

static BOOTSTRAP_ACTIVE: AtomicBool = AtomicBool::new(false);
static SCHEDULER_READY: AtomicBool = AtomicBool::new(false);
static SCHEDULER_TICKS: AtomicU64 = AtomicU64::new(0);
static SCHEDULER: GlobalScheduler = GlobalScheduler::new();
static IDLE_STACK: GlobalIdleStack = GlobalIdleStack::new();

struct GlobalScheduler(UnsafeCell<Scheduler>);

unsafe impl Sync for GlobalScheduler {}

impl GlobalScheduler {
    const fn new() -> Self {
        Self(UnsafeCell::new(Scheduler::new()))
    }

    fn get(&self) -> *mut Scheduler {
        self.0.get()
    }
}

struct GlobalIdleStack(UnsafeCell<AlignedStack>);

unsafe impl Sync for GlobalIdleStack {}

impl GlobalIdleStack {
    const fn new() -> Self {
        Self(UnsafeCell::new(AlignedStack::new()))
    }

    fn get(&self) -> *mut AlignedStack {
        self.0.get()
    }
}

#[repr(align(16))]
struct AlignedStack([u8; KERNEL_STACK_SIZE]);

unsafe impl Sync for AlignedStack {}

impl AlignedStack {
    const fn new() -> Self {
        Self([0; KERNEL_STACK_SIZE])
    }
}

#[derive(Copy, Clone)]
pub struct SchedulerBootstrap {
    pub source: &'static str,
    pub vector: u8,
    pub divide_value: u32,
    pub initial_count: u32,
    pub ticks_observed: u64,
    pub thread_count: usize,
    pub runqueue_depth: usize,
    pub current_thread_id: u64,
    pub current_thread_name: &'static str,
    pub current_thread_role: &'static str,
    pub context_switches: u64,
    pub bootstrap_thread_id: u64,
    pub idle_thread_id: u64,
}

#[derive(Copy, Clone)]
pub struct SchedulerStats {
    pub thread_count: usize,
    pub runqueue_depth: usize,
    pub current_thread_id: u64,
    pub current_thread_name: &'static str,
    pub current_thread_role: &'static str,
    pub current_thread_state: &'static str,
    pub total_ticks: u64,
    pub context_switches: u64,
    pub bootstrap_thread_id: u64,
    pub idle_thread_id: u64,
}

#[derive(Copy, Clone)]
pub struct ExitGroupRecord {
    pub status: i32,
    pub exited_thread_id: u64,
    pub exited_thread_name: &'static str,
    pub next_thread_id: u64,
    pub next_thread_name: &'static str,
    pub runqueue_depth: usize,
}

#[derive(Copy, Clone)]
pub enum SchedulerError {
    Timer(arch::x86_64::TimerError),
    Timeout,
    ThreadTableFull,
    RunQueueFull,
    MissingIdleThread,
}

impl SchedulerError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timer(error) => error.as_str(),
            Self::Timeout => "scheduler bootstrap timed out waiting for periodic timer ticks",
            Self::ThreadTableFull => "scheduler thread table is full",
            Self::RunQueueFull => "scheduler run queue is full",
            Self::MissingIdleThread => "scheduler idle thread is missing",
        }
    }
}

#[derive(Copy, Clone)]
struct Thread {
    id: u64,
    name: &'static str,
    role: ThreadRole,
    state: ThreadState,
    total_ticks: u64,
    dispatch_count: u64,
    context: arch::x86_64::TaskContext,
}

impl Thread {
    const fn empty() -> Self {
        Self {
            id: 0,
            name: "",
            role: ThreadRole::None,
            state: ThreadState::Unused,
            total_ticks: 0,
            dispatch_count: 0,
            context: arch::x86_64::TaskContext::empty(),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum ThreadRole {
    None,
    Bootstrap,
    Idle,
}

impl ThreadRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bootstrap => "bootstrap",
            Self::Idle => "idle",
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum ThreadState {
    Unused,
    Runnable,
    Running,
    Exited,
}

impl ThreadState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unused => "unused",
            Self::Runnable => "runnable",
            Self::Running => "running",
            Self::Exited => "exited",
        }
    }
}

struct Scheduler {
    initialized: bool,
    next_thread_id: u64,
    thread_count: usize,
    threads: [Thread; MAX_THREADS],
    runqueue: [usize; MAX_THREADS],
    runqueue_depth: usize,
    current_runqueue_index: usize,
    context_switches: u64,
    bootstrap_thread_id: u64,
    idle_thread_id: u64,
}

impl Scheduler {
    const fn new() -> Self {
        Self {
            initialized: false,
            next_thread_id: 1,
            thread_count: 0,
            threads: [Thread::empty(); MAX_THREADS],
            runqueue: [0; MAX_THREADS],
            runqueue_depth: 0,
            current_runqueue_index: 0,
            context_switches: 0,
            bootstrap_thread_id: 0,
            idle_thread_id: 0,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn initialize_bootstrap_threads(&mut self) -> Result<(), SchedulerError> {
        self.reset();

        let bootstrap_slot = self.create_thread("kernel-bootstrap", ThreadRole::Bootstrap)?;
        let idle_slot = self.create_thread("kernel-idle", ThreadRole::Idle)?;
        self.enqueue(bootstrap_slot)?;
        self.enqueue(idle_slot)?;

        self.threads[bootstrap_slot].state = ThreadState::Running;
        self.threads[bootstrap_slot].dispatch_count = 1;
        self.threads[idle_slot].state = ThreadState::Runnable;

        let idle_stack = unsafe { &mut (*IDLE_STACK.get()).0 };
        arch::x86_64::initialize_kernel_thread_context(
            &mut self.threads[idle_slot].context,
            idle_stack,
            idle_thread_entry,
        );

        self.current_runqueue_index = 0;
        self.initialized = true;
        self.bootstrap_thread_id = self.threads[bootstrap_slot].id;
        self.idle_thread_id = self.threads[idle_slot].id;

        Ok(())
    }

    fn create_thread(
        &mut self,
        name: &'static str,
        role: ThreadRole,
    ) -> Result<usize, SchedulerError> {
        let slot = self.thread_count;
        if slot >= MAX_THREADS {
            return Err(SchedulerError::ThreadTableFull);
        }

        let thread_id = self.next_thread_id;
        self.next_thread_id = self.next_thread_id.saturating_add(1);
        self.threads[slot] = Thread {
            id: thread_id,
            name,
            role,
            state: ThreadState::Runnable,
            total_ticks: 0,
            dispatch_count: 0,
            context: arch::x86_64::TaskContext::empty(),
        };
        self.thread_count += 1;
        Ok(slot)
    }

    fn enqueue(&mut self, slot: usize) -> Result<(), SchedulerError> {
        if self.runqueue_depth >= MAX_THREADS {
            return Err(SchedulerError::RunQueueFull);
        }

        self.runqueue[self.runqueue_depth] = slot;
        self.runqueue_depth += 1;
        Ok(())
    }

    fn on_timer_tick(&mut self) -> Option<DispatchDecision> {
        if !self.initialized || self.runqueue_depth == 0 {
            return None;
        }

        let current_slot = self.runqueue[self.current_runqueue_index];
        if self.threads[current_slot].state == ThreadState::Running {
            self.threads[current_slot].total_ticks = self.threads[current_slot]
                .total_ticks
                .saturating_add(1);
        }

        if self.runqueue_depth == 1 {
            if self.threads[current_slot].state == ThreadState::Exited {
                return None;
            }
            return Some(self.dispatch_snapshot(current_slot));
        }

        if self.threads[current_slot].state == ThreadState::Running {
            self.threads[current_slot].state = ThreadState::Runnable;
        }
        let mut next_index = (self.current_runqueue_index + 1) % self.runqueue_depth;
        let mut found = false;
        for _ in 0..self.runqueue_depth {
            let candidate = self.runqueue[next_index];
            if self.threads[candidate].state != ThreadState::Exited {
                found = true;
                break;
            }
            next_index = (next_index + 1) % self.runqueue_depth;
        }
        if !found {
            return None;
        }

        self.current_runqueue_index = next_index;
        let next_slot = self.runqueue[next_index];
        self.threads[next_slot].state = ThreadState::Running;
        self.threads[next_slot].dispatch_count = self.threads[next_slot]
            .dispatch_count
            .saturating_add(1);
        if next_slot != current_slot {
            self.context_switches = self.context_switches.saturating_add(1);
        }

        Some(self.dispatch_snapshot(next_slot))
    }

    fn dispatch_snapshot(&self, slot: usize) -> DispatchDecision {
        let thread = self.threads[slot];
        DispatchDecision {
            current_thread_id: thread.id,
            current_thread_name: thread.name,
            current_thread_role: thread.role.as_str(),
            context_switches: self.context_switches,
        }
    }

    fn current_thread(&self) -> Thread {
        if !self.initialized || self.runqueue_depth == 0 {
            return Thread::empty();
        }

        self.threads[self.runqueue[self.current_runqueue_index]]
    }

    fn activate_idle_thread(&mut self) -> Result<(), SchedulerError> {
        if !self.initialized || self.runqueue_depth == 0 || self.idle_thread_id == 0 {
            return Err(SchedulerError::MissingIdleThread);
        }

        let current_slot = self.runqueue[self.current_runqueue_index];
        if self.threads[current_slot].id == self.idle_thread_id {
            self.threads[current_slot].state = ThreadState::Running;
            return Ok(());
        }

        self.threads[current_slot].state = ThreadState::Runnable;
        for index in 0..self.runqueue_depth {
            let slot = self.runqueue[index];
            if self.threads[slot].id == self.idle_thread_id {
                self.current_runqueue_index = index;
                self.threads[slot].state = ThreadState::Running;
                self.context_switches = self.context_switches.saturating_add(1);
                return Ok(());
            }
        }

        Err(SchedulerError::MissingIdleThread)
    }

    fn idle_context_pair(
        &mut self,
    ) -> Result<(&mut arch::x86_64::TaskContext, &arch::x86_64::TaskContext), SchedulerError> {
        self.activate_idle_thread()?;

        let mut bootstrap_slot = None;
        let mut idle_slot = None;
        for slot in 0..self.thread_count {
            match self.threads[slot].role {
                ThreadRole::Bootstrap => bootstrap_slot = Some(slot),
                ThreadRole::Idle => idle_slot = Some(slot),
                ThreadRole::None => {}
            }
        }

        let bootstrap_slot = bootstrap_slot.ok_or(SchedulerError::MissingIdleThread)?;
        let idle_slot = idle_slot.ok_or(SchedulerError::MissingIdleThread)?;
        if bootstrap_slot == idle_slot {
            return Err(SchedulerError::MissingIdleThread);
        }

        let (left, right) = self.threads.split_at_mut(idle_slot.max(bootstrap_slot));
        if bootstrap_slot < idle_slot {
            let current = &mut left[bootstrap_slot].context;
            let next = &right[0].context;
            Ok((current, next))
        } else {
            let next = &left[idle_slot].context;
            let current = &mut right[0].context;
            Ok((current, next))
        }
    }

    fn stats(&self, total_ticks: u64) -> SchedulerStats {
        let current = self.current_thread();
        SchedulerStats {
            thread_count: self.thread_count,
            runqueue_depth: self.runqueue_depth,
            current_thread_id: current.id,
            current_thread_name: current.name,
            current_thread_role: current.role.as_str(),
            current_thread_state: current.state.as_str(),
            total_ticks,
            context_switches: self.context_switches,
            bootstrap_thread_id: self.bootstrap_thread_id,
            idle_thread_id: self.idle_thread_id,
        }
    }

    fn remove_runqueue_index(&mut self, index: usize) {
        if index >= self.runqueue_depth {
            return;
        }
        for cursor in index..self.runqueue_depth.saturating_sub(1) {
            self.runqueue[cursor] = self.runqueue[cursor + 1];
        }
        self.runqueue_depth = self.runqueue_depth.saturating_sub(1);
        if self.runqueue_depth == 0 {
            self.current_runqueue_index = 0;
        } else if self.current_runqueue_index >= self.runqueue_depth {
            self.current_runqueue_index = 0;
        }
    }

    fn request_exit_group(&mut self, status: i32) -> Option<ExitGroupRecord> {
        if !self.initialized || self.runqueue_depth == 0 {
            return None;
        }

        let current_index = self.current_runqueue_index;
        let current_slot = self.runqueue[current_index];
        let current = &mut self.threads[current_slot];
        if current.state == ThreadState::Exited {
            return None;
        }

        current.state = ThreadState::Exited;
        let exited_thread_id = current.id;
        let exited_thread_name = current.name;

        self.remove_runqueue_index(current_index);
        let (next_thread_id, next_thread_name) = if self.runqueue_depth > 0 {
            let next_slot = self.runqueue[self.current_runqueue_index];
            if self.threads[next_slot].state != ThreadState::Exited {
                self.threads[next_slot].state = ThreadState::Running;
            }
            self.context_switches = self.context_switches.saturating_add(1);
            (self.threads[next_slot].id, self.threads[next_slot].name)
        } else {
            (0, "<none>")
        };

        Some(ExitGroupRecord {
            status,
            exited_thread_id,
            exited_thread_name,
            next_thread_id,
            next_thread_name,
            runqueue_depth: self.runqueue_depth,
        })
    }
}

#[derive(Copy, Clone)]
struct DispatchDecision {
    current_thread_id: u64,
    current_thread_name: &'static str,
    current_thread_role: &'static str,
    context_switches: u64,
}

pub fn bootstrap(
    hhdm_offset: u64,
    cpu_info: &arch::x86_64::CpuInfo,
) -> Result<SchedulerBootstrap, SchedulerError> {
    BOOTSTRAP_ACTIVE.store(false, Ordering::Release);
    SCHEDULER_READY.store(false, Ordering::Release);
    SCHEDULER_TICKS.store(0, Ordering::Release);

    unsafe {
        (*SCHEDULER.get()).initialize_bootstrap_threads()?;
    }

    let timer = arch::x86_64::start_local_apic_periodic_timer(hhdm_offset, cpu_info)
        .map_err(SchedulerError::Timer)?;
    let deadline = time::uptime_nanoseconds().saturating_add(BOOTSTRAP_TIMEOUT_NS);

    BOOTSTRAP_ACTIVE.store(true, Ordering::Release);
    enable_interrupts();
    while SCHEDULER_TICKS.load(Ordering::Acquire) < BOOTSTRAP_TARGET_TICKS {
        if time::uptime_nanoseconds() >= deadline {
            BOOTSTRAP_ACTIVE.store(false, Ordering::Release);
            disable_interrupts();
            arch::x86_64::mask_local_apic_timer();
            return Err(SchedulerError::Timeout);
        }

        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
    disable_interrupts();

    BOOTSTRAP_ACTIVE.store(false, Ordering::Release);
    SCHEDULER_READY.store(true, Ordering::Release);

    let stats = unsafe { (*SCHEDULER.get()).stats(SCHEDULER_TICKS.load(Ordering::Acquire)) };
    Ok(SchedulerBootstrap {
        source: "local-apic-periodic",
        vector: timer.vector,
        divide_value: timer.divide_value,
        initial_count: timer.initial_count,
        ticks_observed: stats.total_ticks,
        thread_count: stats.thread_count,
        runqueue_depth: stats.runqueue_depth,
        current_thread_id: stats.current_thread_id,
        current_thread_name: stats.current_thread_name,
        current_thread_role: stats.current_thread_role,
        context_switches: stats.context_switches,
        bootstrap_thread_id: stats.bootstrap_thread_id,
        idle_thread_id: stats.idle_thread_id,
    })
}

pub fn on_timer_interrupt(_apic_tick: u64) {
    if !BOOTSTRAP_ACTIVE.load(Ordering::Acquire) && !SCHEDULER_READY.load(Ordering::Acquire) {
        return;
    }

    let tick = SCHEDULER_TICKS.fetch_add(1, Ordering::AcqRel) + 1;
    let dispatch = unsafe { (*SCHEDULER.get()).on_timer_tick() };
    if tick <= BOOTSTRAP_TARGET_TICKS {
        if let Some(dispatch) = dispatch {
            kprintln!(
                "HXNU: scheduler tick={} current={} role={} id={} switches={}",
                tick,
                dispatch.current_thread_name,
                dispatch.current_thread_role,
                dispatch.current_thread_id,
                dispatch.context_switches,
            );
        } else {
            kprintln!("HXNU: scheduler tick={} current=<none>", tick);
        }
    }
}

pub fn stats() -> SchedulerStats {
    unsafe { (*SCHEDULER.get()).stats(SCHEDULER_TICKS.load(Ordering::Acquire)) }
}

pub fn request_exit_group(status: i32) -> Option<ExitGroupRecord> {
    unsafe { (*SCHEDULER.get()).request_exit_group(status) }
}

pub fn idle_loop() -> ! {
    let (current, next) = unsafe { (*SCHEDULER.get()).idle_context_pair() }
        .unwrap_or_else(|error| panic!("scheduler idle switch failed: {}", error.as_str()));
    unsafe { arch::x86_64::switch_context(current, next) }
}

extern "C" fn idle_thread_entry() -> ! {
    let stats = stats();
    kprintln!(
        "HXNU: scheduler idle loop entered current={} role={} id={}",
        stats.current_thread_name,
        stats.current_thread_role,
        stats.current_thread_id,
    );
    loop {
        unsafe {
            asm!("sti; hlt", options(nomem, nostack));
        }
    }
}

fn enable_interrupts() {
    unsafe {
        asm!("sti", options(nomem, nostack));
    }
}

fn disable_interrupts() {
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
}
