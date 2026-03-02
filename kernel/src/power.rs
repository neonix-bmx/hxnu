use core::arch::asm;
use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::ptr::write_volatile;

use crate::acpi::{FadtInfo, GenericAddress};
use crate::arch;

struct GlobalPower(UnsafeCell<PowerState>);

unsafe impl Sync for GlobalPower {}

impl GlobalPower {
    const fn new() -> Self {
        Self(UnsafeCell::new(PowerState::new()))
    }

    fn get(&self) -> *mut PowerState {
        self.0.get()
    }
}

static POWER_STATE: GlobalPower = GlobalPower::new();

#[derive(Copy, Clone)]
pub struct PowerResetCapability {
    pub supported: bool,
    pub address_space: u8,
    pub address: u64,
    pub value: u8,
}

impl PowerResetCapability {
    pub fn address_space_str(self) -> &'static str {
        match self.address_space {
            0 => "system-memory",
            1 => "system-io",
            2 => "pci-config",
            3 => "embedded-controller",
            4 => "smbus",
            0x0a => "platform-comm",
            0x7f => "functional-fixed",
            _ => "unknown",
        }
    }
}

#[derive(Copy, Clone)]
pub enum ResetError {
    Unsupported,
    UnsupportedAddressSpace,
    MappingFailed,
    NotTriggered,
}

impl ResetError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "power reset register is unavailable",
            Self::UnsupportedAddressSpace => "power reset register address space is unsupported",
            Self::MappingFailed => "failed to map power reset register",
            Self::NotTriggered => "power reset register write did not trigger a reset",
        }
    }
}

struct PowerState {
    hhdm_offset: u64,
    reset_register: Option<GenericAddress>,
    reset_value: u8,
    reset_supported: bool,
}

impl PowerState {
    const fn new() -> Self {
        Self {
            hhdm_offset: 0,
            reset_register: None,
            reset_value: 0,
            reset_supported: false,
        }
    }
}

pub fn configure(hhdm_offset: u64, fadt: &FadtInfo) {
    let state = unsafe { &mut *POWER_STATE.get() };
    state.hhdm_offset = hhdm_offset;
    state.reset_register = fadt.reset_register;
    state.reset_value = fadt.reset_value;
    state.reset_supported = fadt.reset_supported();
}

pub fn reset_capability() -> PowerResetCapability {
    let state = unsafe { &*POWER_STATE.get() };
    match state.reset_register {
        Some(register) => PowerResetCapability {
            supported: state.reset_supported,
            address_space: register.address_space,
            address: register.address,
            value: state.reset_value,
        },
        None => PowerResetCapability {
            supported: false,
            address_space: 0,
            address: 0,
            value: 0,
        },
    }
}

pub fn reboot() -> Result<(), ResetError> {
    let state = unsafe { &*POWER_STATE.get() };
    if !state.reset_supported {
        return Err(ResetError::Unsupported);
    }

    let register = state.reset_register.ok_or(ResetError::Unsupported)?;
    match register.address_space {
        0 => write_reset_mmio(state.hhdm_offset, register, state.reset_value)?,
        1 => write_reset_port(register, state.reset_value)?,
        _ => return Err(ResetError::UnsupportedAddressSpace),
    };

    wait_for_reset();
    try_keyboard_controller_reset();
    wait_for_reset();
    Err(ResetError::NotTriggered)
}

pub fn halt_forever() -> ! {
    loop {
        unsafe {
            asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}

fn write_reset_mmio(hhdm_offset: u64, register: GenericAddress, value: u8) -> Result<(), ResetError> {
    let width_bytes = register_width_bytes(register);
    let virtual_address = arch::x86_64::ensure_physical_region_mapped(
        hhdm_offset,
        register.address,
        width_bytes,
        0,
    )
    .map_err(|_| ResetError::MappingFailed)?;

    unsafe {
        match width_bytes {
            1 => write_volatile(virtual_address as *mut u8, value),
            2 => write_volatile(virtual_address as *mut u16, value as u16),
            4 => write_volatile(virtual_address as *mut u32, value as u32),
            8 => write_volatile(virtual_address as *mut u64, value as u64),
            _ => write_volatile(virtual_address as *mut u8, value),
        }
    }
    Ok(())
}

fn write_reset_port(register: GenericAddress, value: u8) -> Result<(), ResetError> {
    let port = register.address as u16;
    unsafe {
        match register_width_bytes(register) {
            1 if port == 0xcf9 => {
                // Reset Control Register often needs the reset bit to transition low->high.
                let safe_bits = read_port_u8(port) & !0x0e;
                let staged_value = safe_bits | ((value & 0x0e) | 0x02);
                asm!("out dx, al", in("dx") port, in("al") (safe_bits | 0x02), options(nomem, nostack));
                wait_for_reset();
                asm!("out dx, al", in("dx") port, in("al") staged_value, options(nomem, nostack));
            }
            1 => asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack)),
            2 => asm!("out dx, ax", in("dx") port, in("ax") value as u16, options(nomem, nostack)),
            4 => asm!("out dx, eax", in("dx") port, in("eax") value as u32, options(nomem, nostack)),
            _ => asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack)),
        }
    }
    Ok(())
}

fn register_width_bytes(register: GenericAddress) -> usize {
    match register.access_size {
        1 => 1,
        2 => 2,
        3 => 4,
        4 => 8,
        _ => match register.bit_width {
            0..=8 => 1,
            9..=16 => 2,
            17..=32 => 4,
            _ => 8,
        },
    }
}

fn wait_for_reset() {
    for _ in 0..100_000 {
        spin_loop();
    }
}

fn try_keyboard_controller_reset() {
    unsafe {
        asm!("out dx, al", in("dx") 0x64_u16, in("al") 0xfe_u8, options(nomem, nostack));
    }
}

fn read_port_u8(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack));
    }
    value
}
