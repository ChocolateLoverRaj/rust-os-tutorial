#![no_std]
#![no_main]
extern crate alloc;

use frame_buffer_embedded_graphics::*;
use frame_buffer_info::*;
use hhdm_offset::*;
use hlt_loop::*;
use limine_requests::*;
use rgb_pixel_info::*;
use translate_addr::*;

mod frame_buffer_embedded_graphics;
mod frame_buffer_info;
mod hhdm_offset;
mod hlt_loop;
mod limine_requests;
mod logger;
mod memory;
mod panic_handler;
mod rgb_pixel_info;
mod translate_addr;

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point_bsp() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());

    let frame_buffer_response = FRAME_BUFFER_REQUEST.get_response().unwrap();
    logger::init(frame_buffer_response).unwrap();
    log::info!("Hello from BSP");

    let memory_map = MEMORY_MAP_REQUEST.get_response().unwrap();
    // Safety: we are initializing this for the first time
    unsafe { memory::init(memory_map) };

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
