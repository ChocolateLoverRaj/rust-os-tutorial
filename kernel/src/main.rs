#![no_std]
#![no_main]

use limine_requests::{BASE_REVISION, FRAME_BUFFER_REQUEST};

pub mod frame_buffer_embedded_graphics;
pub mod limine_requests;
pub mod logger;

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point_from_limine() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());

    let frame_buffer_response = FRAME_BUFFER_REQUEST.get_response().unwrap();
    logger::init(frame_buffer_response).unwrap();
    log::info!("Hello World!");

    hlt_loop();
}

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("{}", info);
    hlt_loop();
}

fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
