use core::{
    fmt::Debug,
    num::NonZero,
    ops::{Range, RangeInclusive},
    ptr::{NonNull, slice_from_raw_parts_mut},
};

use acpi::{
    AcpiHandler, AcpiTables,
    mcfg::{Mcfg, McfgEntry},
};
use alloc::vec;
use bitfield::{bitfield, bitfield_constructor};
use num_enum::TryFromPrimitive;
use volatile::VolatilePtr;
use x86_64::{PhysAddr, instructions::port::Port, registers::model_specific::PatMemoryType};

use crate::{ConfigurableFlags, Frame, max_page_size, memory::MEMORY};

pub fn get_phys_range_to_map(mcfg_entry: &McfgEntry) -> Range<PhysAddr> {
    let n_buses = (mcfg_entry.bus_number_end - mcfg_entry.bus_number_start) as u64 + 1;
    let start_addr =
        PhysAddr::new(mcfg_entry.base_address + ((mcfg_entry.bus_number_start as u64) << 20));
    let len = n_buses * (1 << 20);
    start_addr..start_addr + len
}

#[derive(Debug)]
pub struct Pci {
    config_address: Port<u32>,
    config_data: Port<u32>,
}

// #[repr(C)]
// #[derive(VolatileFieldAccess)]
// struct PcieConfigurationSpace {
//     vendor_id: u16,
//     device_id: u16,
//     command: u16,
//     status: u16,
//     revision_id: u8,
//     prog_if: u8,
//     sub_class: u8,
//     class_code: u8,
//     cache_line_size: u8,
//     latency_timer: u8,
//     header_type: u8,
//     bist: u8,
//     _padding: [u8; 0xFF0],
// }

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

    fn read_u32(
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
                let mut address = PciConfig::new();
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
}

pub struct PciBus<'a> {
    pci: &'a mut PciAccess,
    bus_number: u8,
}

impl PciBus<'_> {
    pub fn device(&mut self, device_number: u8) -> Option<PciDevice> {
        assert!((0..32).contains(&device_number));
        let vendor_id = self.pci.read_u32(self.bus_number, device_number, 0, 0x0) as u16;
        if vendor_id != u16::MAX {
            let multi_function = HeaderTypeByte(
                (self.pci.read_u32(self.bus_number, device_number, 0, 0xC) >> 16) as u8,
            )
            .multi_function();
            let pci_device = PciDevice {
                pci: self.pci,
                bus_number: self.bus_number,
                device_number,
                multi_function,
            };
            Some(pci_device)
        } else {
            None
        }
    }
}

bitfield! {
  struct PciConfig(u32);
  impl Debug;
  // The fields default to u16
  enable, set_enable: 31;
  u8; bus_number, set_bus_number: 23, 15;
  u8; device_number, set_device_number: 15, 11;
  u8; function_number, set_function_number: 10, 8;
  u8; register_offset, set_register_offset: 7,0 ;
}
impl PciConfig {
    bitfield_constructor!(PciConfig);
}

pub struct PciDevice<'a> {
    pci: &'a mut PciAccess,
    bus_number: u8,
    device_number: u8,
    multi_function: bool,
}

impl PciDevice<'_> {
    pub fn possible_functions(&self) -> RangeInclusive<u8> {
        if self.multi_function { 0..=7 } else { 0..=0 }
    }

    pub fn function(&mut self, function_number: u8) -> Option<PciFunction> {
        assert!((0..=7).contains(&function_number));
        let (vendor_id, device_id) = {
            let reg = self
                .pci
                .read_u32(self.bus_number, self.device_number, function_number, 0x0);
            (reg as u16, (reg >> 16) as u16)
        };
        if vendor_id != u16::MAX {
            let (command, status) = {
                let reg =
                    self.pci
                        .read_u32(self.bus_number, self.device_number, function_number, 0x4);
                (reg as u16, (reg >> 16) as u16)
            };
            let (revision_id, prog_if, sub_class, class_code) = {
                let reg =
                    self.pci
                        .read_u32(self.bus_number, self.device_number, function_number, 0x8);
                (
                    reg as u8,
                    (reg >> 8) as u8,
                    (reg >> 16) as u8,
                    (reg >> 24) as u8,
                )
            };
            let (cache_line_size, latency_timer, header_type, bist) = {
                let reg =
                    self.pci
                        .read_u32(self.bus_number, self.device_number, function_number, 0xC);
                (
                    reg as u8,
                    (reg >> 8) as u8,
                    HeaderTypeByte((reg >> 16) as u8),
                    (reg >> 24) as u8,
                )
            };
            Some(PciFunction {
                pci: self.pci,
                bus_number: self.bus_number,
                device_number: self.device_number,
                function_number: function_number,
                vendor_id,
                device_id,
                command,
                status,
                revision_id,
                prog_if,
                sub_class,
                class_code,
                cache_line_size,
                latency_timer,
                header_type,
                bist,
            })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct PciFunction<'a> {
    pci: &'a mut PciAccess,
    bus_number: u8,
    device_number: u8,
    function_number: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: u16,
    pub status: u16,
    pub revision_id: u8,
    pub prog_if: u8,
    pub sub_class: u8,
    pub class_code: u8,
    pub cache_line_size: u8,
    pub latency_timer: u8,
    pub header_type: HeaderTypeByte,
    pub bist: u8,
}

#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
pub enum HeaderType {
    GeneralDevice = 0x0,
    PciToPciBridge = 0x1,
    PciToCardBusBridge = 0x2,
}

#[derive(Clone, Copy)]
pub struct FullBarMemory {
    bar: MemorySpaceBar,
    next_bar: u32,
}

impl FullBarMemory {
    /// Get the address as a u64, whether it's a 32 bit or 64 bit address
    pub fn addr_u64(&self) -> u64 {
        match self.bar._type() {
            0x0 => (self.bar.0 & !0b1111) as u64,
            0x2 => (self.bar.0 & !0b1111) as u64 | (self.next_bar as u64) << 32,
            _ => unreachable!(),
        }
    }

    pub fn width_bits(&self) -> u8 {
        match self.bar._type() {
            0x0 => 32,
            0x2 => 64,
            _ => unreachable!(),
        }
    }
}

impl Debug for FullBarMemory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MemorySpaceBar")
            .field("width_bits", &self.width_bits())
            .field("addr", &format_args!("0x{:X}", self.addr_u64()))
            .field("prefetchable", &self.bar.prefetchable())
            .finish()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FullBarType {
    Memory(FullBarMemory),
    Io(IoSpaceBar),
}

#[derive(Clone, Copy)]
pub struct FullBar {
    bar: BarCommon,
    next_bar: u32,
}

impl Debug for FullBar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.get_enum().fmt(f)
    }
}

impl FullBar {
    pub fn get_enum(&self) -> FullBarType {
        if self.bar.bar_type() == 0x0 {
            FullBarType::Memory(FullBarMemory {
                bar: MemorySpaceBar(self.bar.0),
                next_bar: self.next_bar,
            })
        } else {
            FullBarType::Io(IoSpaceBar(self.bar.0))
        }
    }

    /// The number of BARs that make up this BAR
    pub fn slots_len(&self) -> u8 {
        if self.bar.bar_type() == 0x0 {
            if MemorySpaceBar(self.bar.0)._type() == 0x2 {
                2
            } else {
                1
            }
        } else {
            1
        }
    }
}

bitfield! {
    #[derive( Clone, Copy)]
  pub struct BarCommon(u32);
  impl Debug;
  u8; bar_type, _: 1, 1;
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct MemorySpaceBar(u32);
  impl Debug;
    prefetchable, _: 3;
    u8; _type, _: 2, 1;
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct IoSpaceBar(u32);
}

impl IoSpaceBar {
    pub fn addr(self) -> u32 {
        // The lowest 2 bits should be masked out
        self.0 & !0b11
    }
}

impl Debug for IoSpaceBar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IoSpaceBar")
            .field("addr", &format_args!("0x{:X}", self.addr()))
            .finish()
    }
}

impl PciFunction<'_> {
    pub fn header_type(&self) -> Option<HeaderType> {
        self.header_type.header_type().try_into().ok()
    }

    pub fn max_bars(&self) -> u8 {
        match self.header_type().unwrap() {
            HeaderType::GeneralDevice => 6,
            HeaderType::PciToPciBridge => 2,
            HeaderType::PciToCardBusBridge => 0,
        }
    }

    fn read_bar_raw(&mut self, bar_index: u8) -> u32 {
        assert!((0..self.max_bars()).contains(&bar_index));
        self.pci.read_u32(
            self.bus_number,
            self.device_number,
            self.function_number,
            0x10 + size_of::<u32>() as u8 * bar_index,
        )
    }

    pub fn read_bar(&mut self, bar_index: u8) -> Option<FullBar> {
        let bar0 = NonZero::new(self.read_bar_raw(bar_index))?.get();
        Some(if BarCommon(bar0).bar_type() == 0x0 {
            let bar = MemorySpaceBar(bar0);
            match bar._type() {
                0x0 => FullBar {
                    bar: BarCommon(bar0),
                    next_bar: Default::default(),
                },
                0x2 => FullBar {
                    bar: BarCommon(bar0),
                    next_bar: self.read_bar_raw(bar_index + 1),
                },
                bar_type => panic!("unsupported bar type: 0x{bar_type:X}"),
            }
        } else {
            FullBar {
                bar: BarCommon(bar0),
                next_bar: Default::default(),
            }
        })
    }
}

bitfield! {
  pub struct HeaderTypeByte(u8);
  impl Debug;
  // The fields default to u16
  multi_function, _: 7;
  u8; header_type, _: 6, 0;
}

pub fn init(acpi_tables: &AcpiTables<impl AcpiHandler>) {
    let pci_vec = if let Ok(mcfg) = acpi_tables.find_table::<Mcfg>() {
        let memory = MEMORY.get().unwrap();
        let mut virt_mem = memory.virtual_memory.lock();
        let mut phys_mem = memory.physical_memory.lock();
        let mut frame_allocator = phys_mem.get_kernel_frame_allocator();
        mcfg.entries()
            .iter()
            .map(|entry| {
                let range = get_phys_range_to_map(entry);
                let page_size = max_page_size();
                let offset_in_page = range.start.as_u64() % page_size.byte_len_u64();
                let first_frame = Frame::new(range.start - offset_in_page, page_size).unwrap();
                let n_pages = range.end.as_u64().div_ceil(page_size.byte_len_u64())
                    - range.start.as_u64() / page_size.byte_len_u64();
                let mut pages = virt_mem
                    .allocate_contiguous_pages(page_size, n_pages)
                    .unwrap();
                for i in 0..n_pages {
                    let page = pages.start_page().offset(i).unwrap();
                    let frame = first_frame.offset(i).unwrap();
                    let flags = ConfigurableFlags {
                        writable: true,
                        executable: false,
                        pat_memory_type: PatMemoryType::StrongUncacheable,
                    };
                    unsafe { pages.map_to(page, frame, flags, &mut frame_allocator) }.unwrap();
                }
                let mapped_mem = NonNull::new(slice_from_raw_parts_mut(
                    (pages.start_addr() + offset_in_page).as_mut_ptr(),
                    (range.end - range.start) as usize,
                ))
                .unwrap();
                unsafe { PciAccess::new_pcie(*entry, mapped_mem) }
            })
            .collect()
    } else {
        vec![unsafe { PciAccess::new_pci() }]
    };
    for mut pci in pci_vec {
        for bus_number in pci.known_buses() {
            let mut bus = pci.bus(bus_number);
            for device_number in 0..32 {
                if let Some(mut device) = bus.device(device_number) {
                    for function_number in device.possible_functions() {
                        if let Some(mut function) = device.function(function_number) {
                            log::debug!("{function:#X?}");
                            let mut bar_number = 0;
                            while bar_number < function.max_bars() {
                                if let Some(bar) = function.read_bar(bar_number) {
                                    log::debug!("Bar {bar_number}: {bar:?}");
                                    bar_number += bar.slots_len();
                                } else {
                                    log::debug!("No bar {bar_number}");
                                    bar_number += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
