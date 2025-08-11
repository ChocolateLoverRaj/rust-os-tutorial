use core::{fmt::Debug, ops::RangeInclusive, ptr::NonNull};

use acpi::mcfg::McfgEntry;
use volatile::VolatilePtr;
use x86_64::instructions::port::Port;

use super::{PciBus, PciConfig};

#[derive(Debug)]
pub struct Pci {
    config_address: Port<u32>,
    config_data: Port<u32>,
}

#[derive(Debug)]
pub struct Pcie {
    mcfg_entry: McfgEntry,
    ptr: VolatilePtr<'static, [u8]>,
}

#[derive(Debug)]
pub enum PciAccess {
    Pci(Pci),
    Pcie(Pcie),
}

impl PciAccess {
    /// # Safety
    /// The ports must be PCI and not used by other code.
    pub unsafe fn new_pci() -> Self {
        Self::Pci(Pci {
            config_address: Port::<u32>::new(0xCF8),
            config_data: Port::<u32>::new(0xCFC),
        })
    }

    /// # Safety
    /// The mapped mem must point to physical memory for the MCFG entry, which you can calculate using [`get_phys_range_to_map`].
    pub unsafe fn new_pcie(mcfg_entry: McfgEntry, mapped_mem: NonNull<[u8]>) -> Self {
        Self::Pcie(Pcie {
            mcfg_entry,
            ptr: unsafe { VolatilePtr::new(mapped_mem) },
        })
    }

    pub fn known_buses(&self) -> RangeInclusive<u8> {
        match self {
            Self::Pci(_) => 0..=0,
            Self::Pcie(pcie) => pcie.mcfg_entry.bus_number_start..=pcie.mcfg_entry.bus_number_end,
        }
    }

    pub fn bus(&mut self, bus_number: u8) -> PciBus {
        PciBus {
            pci: self,
            bus_number,
        }
    }

    pub(super) fn read_u32(
        &mut self,
        bus_number: u8,
        device_number: u8,
        function_number: u8,
        register_offset: u8,
    ) -> u32 {
        assert!(
            register_offset.is_multiple_of(size_of::<u32>().try_into().unwrap()),
            "Register offset represents bytes and should be aligned to u32"
        );
        match self {
            Self::Pci(pci) => {
                let mut address = PciConfig(0);
                address.set_enable(true);
                address.set_bus_number(bus_number);
                address.set_device_number(device_number);
                address.set_function_number(function_number);
                address.set_register_offset(register_offset);

                unsafe { pci.config_address.write(address.0) };
                unsafe { pci.config_data.read() }
            }
            Self::Pcie(pcie) => {
                // assert!(self.known_buses().contains(&bus_number));
                let bus_offset = bus_number - pcie.mcfg_entry.bus_number_start;
                let bytes = pcie
                    .ptr
                    .as_chunks()
                    .0
                    .index(
                        ((bus_offset as usize) << 20
                            | (device_number as usize) << 15
                            | (function_number as usize) << 12
                            | register_offset as usize)
                            / 4,
                    )
                    .read();
                u32::from_le_bytes(bytes)
            }
        }
    }

    pub(super) fn write_u32(
        &mut self,
        bus_number: u8,
        device_number: u8,
        function_number: u8,
        register_offset: u8,
        value: u32,
    ) {
        assert!(
            register_offset.is_multiple_of(size_of::<u32>().try_into().unwrap()),
            "Register offset represents bytes and should be aligned to u32"
        );
        match self {
            Self::Pci(pci) => {
                let mut address = PciConfig(0);
                address.set_enable(true);
                address.set_bus_number(bus_number);
                address.set_device_number(device_number);
                address.set_function_number(function_number);
                address.set_register_offset(register_offset);

                unsafe { pci.config_address.write(address.0) };
                unsafe { pci.config_data.write(value) }
            }
            Self::Pcie(pcie) => {
                // assert!(self.known_buses().contains(&bus_number));
                let bus_offset = bus_number - pcie.mcfg_entry.bus_number_start;
                pcie.ptr
                    .as_chunks()
                    .0
                    .index(
                        ((bus_offset as usize) << 20
                            | (device_number as usize) << 15
                            | (function_number as usize) << 12
                            | register_offset as usize)
                            / 4,
                    )
                    .write(value.to_le_bytes());
            }
        }
    }
}
