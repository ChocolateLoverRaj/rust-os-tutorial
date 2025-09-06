# Parsing ACPI Tables
ACPI tables is binary data that provides information about the computer to the operating system. We'll need to parse ACPI tables to send interrupts between CPUs, access timers, and more. We'll use the `acpi` crate, which parses the binary data into nice Rust types.
```toml
acpi = "6.0.1"
```
Create a file `acpi.rs`. We'll be using the [`AcpiTables::from_rsdp`](https://docs.rs/acpi/latest/acpi/struct.AcpiTables.html#method.from_rsdp) method. It needs a handler, which maps the ACPI memory, and the address of the [RSDP](https://wiki.osdev.org/RSDP). 

## RSDP request
We can ask Limine for the RSDP address by adding the request:
```rs
#[used]
#[unsafe(link_section = ".requests")]
pub static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();
```

## Implementing `acpi::Handler`
For the handler, we'll need to make our own. In `acpi.rs`, add:
```rs
/// Note: this cannot be sent across CPUs because the other CPUs did not flush their cache for changes in page tables
#[derive(Debug, Clone)]
struct KernelAcpiHandler {
    phantom: PhantomData<NonNull<()>>,
}

impl acpi::Handler for KernelAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        todo!()
    }

    fn unmap_physical_region<T>(region: &acpi::PhysicalMapping<Self, T>) {
        todo!()
    }

    // We don't actually need the following functions
    fn read_u8(&self, address: usize) -> u8 {
        let _ = address;
        unimplemented!()
    }

    fn read_u16(&self, address: usize) -> u16 {
        let _ = address;
        unimplemented!()
    }

    fn read_u32(&self, address: usize) -> u32 {
        let _ = address;
        unimplemented!()
    }

    fn read_u64(&self, address: usize) -> u64 {
        let _ = address;
        unimplemented!()
    }

    fn write_u8(&self, address: usize, value: u8) {
        let _ = value;
        let _ = address;
        unimplemented!()
    }

    fn write_u16(&self, address: usize, value: u16) {
        let _ = address;
        let _ = value;
        unimplemented!()
    }

    fn write_u32(&self, address: usize, value: u32) {
        let _ = address;
        let _ = value;
        unimplemented!()
    }

    fn write_u64(&self, address: usize, value: u64) {
        let _ = address;
        let _ = value;
        unimplemented!()
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        let _ = port;
        unimplemented!()
    }

    fn read_io_u16(&self, port: u16) -> u16 {
        let _ = port;
        unimplemented!()
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        let _ = port;
        unimplemented!()
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        let _ = port;
        let _ = value;
        unimplemented!()
    }

    fn write_io_u16(&self, port: u16, value: u16) {
        let _ = port;
        let _ = value;
        unimplemented!()
    }

    fn write_io_u32(&self, port: u16, value: u32) {
        let _ = port;
        let _ = value;
        unimplemented!()
    }

    fn read_pci_u8(&self, address: acpi::PciAddress, offset: u16) -> u8 {
        let _ = address;
        let _ = offset;
        unimplemented!()
    }

    fn read_pci_u16(&self, address: acpi::PciAddress, offset: u16) -> u16 {
        let _ = address;
        let _ = offset;
        unimplemented!()
    }

    fn read_pci_u32(&self, address: acpi::PciAddress, offset: u16) -> u32 {
        let _ = address;
        let _ = offset;
        unimplemented!()
    }

    fn write_pci_u8(&self, address: acpi::PciAddress, offset: u16, value: u8) {
        let _ = address;
        let _ = offset;
        let _ = value;
        unimplemented!()
    }

    fn write_pci_u16(&self, address: acpi::PciAddress, offset: u16, value: u16) {
        let _ = address;
        let _ = offset;
        let _ = value;
        unimplemented!()
    }

    fn write_pci_u32(&self, address: acpi::PciAddress, offset: u16, value: u32) {
        let _ = address;
        let _ = offset;
        let _ = value;
        unimplemented!()
    }

    fn nanos_since_boot(&self) -> u64 {
        unimplemented!()
    }

    fn stall(&self, microseconds: u64) {
        let _ = microseconds;
        unimplemented!()
    }

    fn sleep(&self, milliseconds: u64) {
        let _ = milliseconds;
        unimplemented!()
    }

    fn create_mutex(&self) -> acpi::Handle {
        unimplemented!()
    }

    fn acquire(&self, mutex: acpi::Handle, timeout: u16) -> Result<(), acpi::aml::AmlError> {
        let _ = mutex;
        let _ = timeout;
        unimplemented!()
    }

    fn release(&self, mutex: acpi::Handle) {
        let _ = mutex;
        unimplemented!()
    }
}
```
We use `PhantomData<NonNull<()>>` to mark `KernelAcpiHandler` as not being able to be sent across CPUs. This is because page mappings are not synchronized between CPUs unless we flush the changed pages from the [TLB](https://en.wikipedia.org/wiki/Translation_lookaside_buffer).

We will only need to implement `map_physical_region` and `unmap_physical_region`. The rest of the functions will not get called. 

### Implementing `map_physical_region`
In `map_physical_region`, we need to:
- Find an unused virtual memory range
- Map the physical memory to the virtual memory range
- Return the mapping information

Add the following code:
```rs
let page_size = max_page_size();
let memory = MEMORY.get().unwrap();
let mut virtual_memory = memory.virtual_memory.lock();
let n_pages = ((size + physical_address) as u64).div_ceil(page_size.byte_len_u64())
    - physical_address as u64 / page_size.byte_len_u64();
let start_page = virtual_memory
    .allocate_contiguous_pages(
        page_size,
        NonZero::new(n_pages).expect("at least 1 byte mapped"),
    )
    .unwrap();
```
Here we will use the largest supported page size. It's okay if we map extra bytes, and we can improve performance by using fewer, bigger mappings. Then, we use `allocate_contiguous_pages` to allocate virtual memory to create the mapping at.

Next, we do the actual mapping:
```rs
let start_frame = Frame::new(
    PhysAddr::new(
        physical_address as u64 / page_size.byte_len_u64() * page_size.byte_len_u64(),
    ),
    page_size,
)
.unwrap();
let mut physical_memory = memory.physical_memory.lock();
let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
for i in 0..n_pages {
    let page = start_page.offset(i).unwrap();
    let frame = start_frame.offset(i).unwrap();
    let flags = ConfigurableFlags {
        executable: false,
        writable: false,
        pat_memory_type: PatMemoryType::WriteBack,
    };
    unsafe {
        virtual_memory
            .l4_mut()
            .map_page(page, frame, flags, &mut frame_allocator)
            .unwrap();
    }
}
```
We specify that the memory is not writable, to page fault if the code for some reason tries to write to the ACPI tables. We specify that it is not executable, since ACPI tables don't have any code that we should execute. We use `PatMemoryType::WriteBack` because it is the most performant memory type, and ACPI tables don't have any read or write side effects. 

Finally, we return a `PhysicalMapping`:
```rs
PhysicalMapping {
    physical_start: physical_address,
    virtual_start: NonNull::new(
        (start_page.start_addr() + physical_address as u64 % page_size.byte_len_u64())
            .as_mut_ptr(),
    )
    .unwrap(),
    region_length: size,
    mapped_length: n_pages as usize * page_size.byte_len(),
    handler: self.clone(),
}
```
Note that because the physical address we mapped isn't necessarily aligned, we have to do some math to find the correct `virtual_start` that maps to the requested `physical_start`. 

## Implementing `unmap_physical_region`
```rs
let page_size = max_page_size();
let start_page = Page::new(
    VirtAddr::from_ptr(region.virtual_start.as_ptr()).align_down(page_size.byte_len_u64()),
    page_size,
)
.unwrap();
let mut virtual_memory = MEMORY.get().unwrap().virtual_memory.lock();
let n_pages = region.mapped_length as u64 / page_size.byte_len_u64();
for i in 0..n_pages {
    let page = start_page.offset(i).unwrap();
    unsafe { virtual_memory.l4_mut().unmap_page(page) }.unwrap();
}
```
We again get the same page size, `start_page`, and `n_pages` from the `PhysicalMapping`, and then unmap the pages.

## Using our ACPI handler
In `acpi.rs`, add:
```rs
pub fn parse(rsdp: &RsdpResponse) -> AcpiTables<impl acpi::Handler> {
    let address = rsdp.address();
    unsafe {
        AcpiTables::from_rsdp(
            KernelAcpiHandler {
                phantom: PhantomData,
            },
            address,
        )
    }
    .unwrap()
}
```
Then, in `main.rs`, after calling `idt::init()`, add:
```rs
let rsdp = RSDP_REQUEST.get_response().unwrap();
let acpi_tables = acpi::parse(rsdp)
    .table_headers()
    .map(|(_, header)| header.signature)
    .collect::<alloc::boxed::Box<[_]>>();
log::info!("ACPI Tables: {acpi_tables:?}");
```
This should log:
```
[0] INFO  ACPI Tables: ["FACP", "APIC", "HPET", "WAET", "BGRT"]
```
We definitely will be using information from the `APIC` and `HPET` ACPI tables in the future, so it's good that we are able to successfully parse those tables.

## ACPI tables on real hardware
### Jinlon
- FACP
- SSDT
- MCFG
- TPM2
- LPIT
- APIC
- SPCR
- DMAR
- DBG2
- HPET
- BGRT

### Lenovo Z560
- FACP
- ASF!
- HPET
- APIC
- MCFG
- SLIC
- BOOT
- ASPT
- WDRT
- SSDT
- SSDT
- SSDT

# Learn More
- https://wiki.osdev.org/RSDP
- https://wiki.osdev.org/ACPI
