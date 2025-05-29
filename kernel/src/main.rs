#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

use frame_buffer_embedded_graphics::*;
use frame_buffer_info::*;
use gdt::*;
use hhdm_offset::*;
use hlt_loop::*;
use limine_requests::*;
use rgb_pixel_info::*;
use translate_addr::*;

mod cpu_local_data;
mod frame_buffer_embedded_graphics;
mod frame_buffer_info;
mod gdt;
mod hhdm_offset;
mod hlt_loop;
mod idt;
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

    // Safety: We are calling this function on the BSP
    unsafe {
        cpu_local_data::init_bsp();
    }

    let mp_response = MP_REQUEST.get_response().unwrap();
    for cpu in mp_response.cpus() {
        cpu.goto_address.write(entry_point_ap);
    }

    gdt::init();
    idt::init();

    hlt_loop();
}

unsafe extern "C" fn entry_point_ap(cpu: &limine::mp::Cpu) -> ! {
    // Safety: We're actually calling the function on this CPU
    unsafe { cpu_local_data::init_ap(cpu) };

    log::info!("Hello from AP");

    gdt::init();
    idt::init();

    hlt_loop()
}
