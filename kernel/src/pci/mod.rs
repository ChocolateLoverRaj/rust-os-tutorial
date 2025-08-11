use core::fmt::Debug;

pub use bar::*;
pub use bus::*;
pub use device::*;
pub use function::*;
pub use get_phys_range_to_map::*;
pub use header_type_byte::*;
use num_enum::TryFromPrimitive;
pub use pci_access::*;
use pci_config::*;

mod bar;
mod bus;
mod device;
mod function;
mod get_phys_range_to_map;
mod header_type_byte;
mod pci_access;
mod pci_config;

#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
pub enum HeaderType {
    GeneralDevice = 0x0,
    PciToPciBridge = 0x1,
    PciToCardBusBridge = 0x2,
}
