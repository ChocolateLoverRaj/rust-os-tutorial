use core::sync::atomic::{AtomicBool, Ordering};

use crate::hlt_loop::hlt_loop;

static DID_PANIC: AtomicBool = AtomicBool::new(false);
#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    match DID_PANIC.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed) {
        Ok(_) => {
            log::error!("{}", info);
            hlt_loop();
        }
        Err(_) => {
            hlt_loop();
        }
    }
}
