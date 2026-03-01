use core::alloc::Layout;
use core::arch::asm;
use core::panic::PanicInfo;

use crate::serial;

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    serial::init();
    kprintln!("HXNU: ==================== KERNEL PANIC ====================");
    if let Some(location) = info.location() {
        kprintln!(
            "HXNU: location  {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    } else {
        kprintln!("HXNU: location  <unknown>");
    }
    kprintln!("HXNU: message   {}", info.message());
    kprintln!("HXNU: action    cpu halted");
    kprintln!("HXNU: =====================================================");

    loop {
        unsafe {
            asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}

#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    serial::init();
    kprintln!("HXNU: ================= ALLOCATION FAILURE =================");
    kprintln!("HXNU: size      {}", layout.size());
    kprintln!("HXNU: align     {}", layout.align());
    kprintln!("HXNU: action    cpu halted");
    kprintln!("HXNU: =====================================================");

    loop {
        unsafe {
            asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}
