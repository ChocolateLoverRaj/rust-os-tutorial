#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

use call_with_rsp::*;
use cpu_local_data::get_local;
use elf_segment_flags::*;
use elf_segment_type::*;
use enter_user_mode::*;
use frame_buffer_embedded_graphics::*;
use frame_buffer_info::*;
use gdt::*;
use guarded_stack::*;
use hhdm_offset::*;
use hlt_loop::*;
use interrupt_vector::*;
use limine_requests::*;
use rgb_pixel_info::*;
use run_program_0::*;
use translate_addr::*;
use writer_with_cr::*;
use x86_64_consts::*;

mod acpi;
mod apic;
mod call_with_rsp;
mod cpu_local_data;
mod elf_segment_flags;
mod elf_segment_type;
mod enter_user_mode;
mod frame_buffer_embedded_graphics;
mod frame_buffer_info;
mod gdt;
mod guarded_stack;
mod hhdm_offset;
mod hlt_loop;
mod idt;
mod interrupt_vector;
mod limine_requests;
mod logger;
mod memory;
mod nmi_handler_states;
mod panic_handler;
mod rgb_pixel_info;
mod run_program_0;
mod spcr;
mod syscall_handler;
mod translate_addr;
mod writer_with_cr;
mod x86_64_consts;

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point_bsp() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());

    let frame_buffer_response = FRAME_BUFFER_REQUEST.get_response().unwrap();
    logger::init(frame_buffer_response).unwrap();
    log::info!("Hello from BSP");

    let memory_map = MEMORY_MAP_REQUEST.get_response().unwrap();
    // Safety: no page tables were modified before this
    unsafe { memory::init_bsp(memory_map) };

    // Safety: We are calling this function on the BSP
    unsafe {
        cpu_local_data::init_bsp();
    }

    GuardedStack::new(
        NORMAL_STACK_SIZE,
        StackId {
            _type: StackType::Normal,
            cpu_id: get_local().kernel_assigned_id,
        },
    )
    .switch(init_bsp)
}

extern "sysv64" fn init_bsp() -> ! {
    nmi_handler_states::init();

    gdt::init();
    idt::init();

    let rsdp = RSDP_REQUEST.get_response().unwrap();
    let acpi_tables = acpi::parse(rsdp);
    spcr::init(&acpi_tables);
    apic::init_bsp(&acpi_tables);
    apic::init_local_apic();

    let mp_response = MP_REQUEST.get_response().unwrap();
    for cpu in mp_response.cpus() {
        cpu.goto_address.write(entry_point_ap);
    }

    run_program_0();

    hlt_loop();
}

unsafe extern "C" fn entry_point_ap(cpu: &limine::mp::Cpu) -> ! {
    // Safety: we are calling this right away
    unsafe { memory::init_ap() };
    // Safety: We're actually calling the function on this CPU
    unsafe { cpu_local_data::init_ap(cpu) };

    log::info!("Hello from AP");

    GuardedStack::new(
        NORMAL_STACK_SIZE,
        StackId {
            _type: StackType::Normal,
            cpu_id: get_local().kernel_assigned_id,
        },
    )
    .switch(init_ap)
}

extern "sysv64" fn init_ap() -> ! {
    gdt::init();
    idt::init();
    apic::init_local_apic();

    hlt_loop()
}
