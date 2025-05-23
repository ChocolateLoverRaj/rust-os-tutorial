#![no_std]
#![no_main]
#![feature(sync_unsafe_cell)]

extern crate alloc;

use hlt_loop::hlt_loop;
use limine_requests::{BASE_REVISION, MP_REQUEST};

pub mod global_allocator;
pub mod hlt_loop;
pub mod limine_requests;
pub mod logger;
pub mod panic_handler;

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point_from_limine() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());

    logger::init().unwrap();
    log::info!("Hello World!");

    // Safety: we are initializing this for the first time
    unsafe { global_allocator::init() };

    let mp_response = MP_REQUEST.get_response().unwrap();
    let cpu_count = mp_response.cpus().len();
    log::info!("CPU Count: {}", cpu_count);
    for cpu in mp_response.cpus() {
        cpu.goto_address.write(entry_point_from_limine_mp);
    }

    todo!()
}

unsafe extern "C" fn entry_point_from_limine_mp(cpu: &limine::mp::Cpu) -> ! {
    let cpu_id = cpu.id;
    log::info!("Hello from CPU {}", cpu_id);
    hlt_loop()
}
