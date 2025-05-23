#![no_std]
#![no_main]

extern crate alloc;

use cpu_local_data::init_cpu;
use hlt_loop::hlt_loop;
use limine_requests::{BASE_REVISION, HHDM_REQUEST, MEMORY_MAP_REQUEST, MP_REQUEST};
use memory::MEMORY;
use x86_64::registers::control::Cr3;

pub mod cpu_local_data;
pub mod cut_range;
pub mod hhdm_offset;
pub mod hlt_loop;
pub mod initial_frame_allocator;
pub mod initial_usable_frames_iterator;
pub mod limine_requests;
pub mod logger;
pub mod memory;
pub mod panic_handler;

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point_from_limine() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());

    logger::init().unwrap();
    log::info!("Hello World!");

    let memory_map = MEMORY_MAP_REQUEST.get_response().unwrap();
    let hhdm_offset = HHDM_REQUEST.get_response().unwrap().into();
    // Safety: we are initializing this for the first time
    unsafe { memory::init(memory_map, hhdm_offset) };

    let mp_response = MP_REQUEST.get_response().unwrap();
    let cpu_count = mp_response.cpus().len();
    log::info!("CPU Count: {}", cpu_count);
    cpu_local_data::init(mp_response);
    unsafe {
        init_cpu(
            mp_response,
            mp_response
                .cpus()
                .iter()
                .find(|cpu| cpu.lapic_id == mp_response.bsp_lapic_id())
                .unwrap(),
        );
    }
    for cpu in mp_response.cpus() {
        cpu.goto_address.write(entry_point_from_limine_mp);
    }

    log::info!("Hello from BSP");

    hlt_loop();
}

unsafe extern "C" fn entry_point_from_limine_mp(cpu: &limine::mp::Cpu) -> ! {
    let memory = MEMORY.try_get().unwrap();
    // Safety: This function is only executed after memory is initialized
    unsafe { Cr3::write(memory.new_kernel_cr3, memory.new_kernel_cr3_flags) };
    // Safety: We're inputting the correct CPU
    unsafe { init_cpu(MP_REQUEST.get_response().unwrap(), cpu) };

    let cpu_id = cpu.id;
    log::info!("Hello from CPU {}", cpu_id);
    hlt_loop()
}
