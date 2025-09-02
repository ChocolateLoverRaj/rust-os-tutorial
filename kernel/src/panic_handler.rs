use core::sync::atomic::{AtomicBool, Ordering};

use crate::hlt_loop::hlt_loop;

static DID_PANIC: AtomicBool = AtomicBool::new(false);
#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    if !DID_PANIC.swap(true, Ordering::Relaxed) {
        log::error!("{info}");
        hlt_loop();
    } else {
        hlt_loop();
    }
}
