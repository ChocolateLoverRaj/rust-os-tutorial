#![no_std]

mod arch;
mod logger;

pub use arch::Arch;
pub use logger::EarlyLogger;

use log::info;

use crate::arch::ARCH;

pub enum BootInfo {
    FdtAddr(usize),
}

pub fn start(arch: Arch, boot_info: BootInfo) -> ! {
    let arch = ARCH.call_once(|| arch);
    logger::init();

    info!("Hello from Rust kernel");

    match boot_info {
        BootInfo::FdtAddr(addr) => {
            let fdt = {
                let ptr = fdt_ptr as *mut _;
                unsafe { Fdt::from_ptr(ptr) }
            }
            .unwrap();

            let cpu_node = fdt.find_by_path("/cpus/cpu@0").unwrap();
            let extensions = cpu_node.find_property("riscv,isa-extensions").unwrap();
            for extension in extensions.as_str_iter() {
                info!("RISC-V extension: {extension:?}");
            }

            for node in fdt.all_nodes() {
                let node_path = node.path();
                info!("node: {node_path:?}");
            }

            for memory in fdt.memory() {
                for reg in memory.reg().unwrap() {
                    info!("reg: {reg:#X?}");
                }
            }

            for memory_reservation in fdt.memory_reservations() {
                info!("memory reservation: {memory_reservation:#X?}");
            }

            for node in fdt.reserved_memory() {
                for reg in node.reg().unwrap() {
                    info!("reserved mem: {reg:#X?}");
                }
            }
        }
    }

    if let Some(shutdown) = arch.shutdown {
        shutdown()
    } else {
        (arch.low_power_loop)()
    }
}
