use core::ops::Range;

pub use acpi::mcfg::McfgEntry;
pub use x86_64::PhysAddr;

pub fn get_phys_range_to_map(mcfg_entry: &McfgEntry) -> Range<PhysAddr> {
    let n_buses = (mcfg_entry.bus_number_end - mcfg_entry.bus_number_start) as u64 + 1;
    let start_addr =
        PhysAddr::new(mcfg_entry.base_address + ((mcfg_entry.bus_number_start as u64) << 20));
    let len = n_buses * (1 << 20);
    start_addr..start_addr + len
}
