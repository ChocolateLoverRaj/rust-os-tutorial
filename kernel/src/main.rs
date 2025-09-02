#![no_std]
#![no_main]

use frame_buffer_embedded_graphics::*;
use frame_buffer_info::*;
use hlt_loop::*;
use limine_requests::*;
use rgb_pixel_info::*;

mod frame_buffer_embedded_graphics;
mod frame_buffer_info;
mod hlt_loop;
mod limine_requests;
mod logger;
mod panic_handler;
mod rgb_pixel_info;

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point_bsp() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());

    let frame_buffer_response = FRAME_BUFFER_REQUEST.get_response().unwrap();
    logger::init(frame_buffer_response).unwrap();
    log::info!("Hello from BSP");

    let mp_response = MP_REQUEST.get_response().unwrap();
    for cpu in mp_response.cpus() {
        cpu.goto_address.write(entry_point_ap);
    }

    hlt_loop();
}

unsafe extern "C" fn entry_point_ap(_cpu: &limine::mp::Cpu) -> ! {
    log::info!("Hello from AP");
    hlt_loop()
}
