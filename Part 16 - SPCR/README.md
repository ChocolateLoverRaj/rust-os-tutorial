# Serial Port Console Redirect (SPCR)
It can be really hard to debug when things don't work on real devices. We can log things to the screen, but we are limited to what we can see on the screen. We already log to COM1, but we can only see the it in a virtual machine. Fortunately, some computers, such as Chromebooks, do have a "serial port". There is an ACPI table, [SPCR](https://learn.microsoft.com/en-us/windows-hardware/drivers/bringup/serial-port-console-redirection-table), which defines how to access a serial port. 

## Chromebooks
See https://docs.mrchromebox.tech/docs/support/debugging.html#suzyqable-debug-cable. Basically, you need to be running MrChromebox UEFI firmware with the coreboot flags `CONSOLE_SERIAL=y` and `EDK2_SERIAL_SUPPORT=y`, and have a SuzyQable Debug Cable. You can [make a debug cable](https://chromium.googlesource.com/chromiumos/third_party/hdctools/+/main/docs/ccd.md#SuzyQ-SuzyQable) or [buy one](https://github.com/ChocolateLoverRaj/gsc-debug-board/). Remember, this cable is only for certain **Chromebooks**, don't try to use it on other laptops.

## Other computers
Since SPCR is a standard ACPI table, there is a chance that the code in this tutorial will just work. But you might have to modify it depending on whats in the SPCR table.

## Parsing the SPCR
Create a file `spcr.rs`. Again, we will be mapping page tables, so we're going to ake a generic function that takes `PageSize`. We will also add the `uart` crate, which let's us set a custom baud rate, which is needed on Chromebooks:
```toml
uart = { git = "https://github.com/ChocolateLoverRaj/uart", branch = "send-sync" }
```
```rs
/// Checks for SPCR, and sets logger to log through SPCR instead of COM1 accordingly
pub fn init(acpi_tables: &AcpiTables<impl acpi::Handler>) {
    let page_size = max_page_size();
    if let Some(uart) = acpi_tables
        .find_table::<Spcr>()
        // The table might not exist
        .ok()
        .and_then(|spcr| {
            // We may not know how to handle the interface type
            match spcr.interface_type() {
                // These 3 can be handled by the uart crate
                SpcrInterfaceType::Full16550
                | SpcrInterfaceType::Full16450
                | SpcrInterfaceType::Generic16550 => spcr.base_address(),
                _ => None,
            }
        })
        // We get the base address, which is how we access the uart
        .and_then(|base_address| base_address.ok())
        // https://uefi.org/htmlspecs/ACPI_Spec_6_4_html/05_ACPI_Software_Programming_Model/ACPI_Software_Programming_Model.html#generic-address-structure-gas
        // ACPI addresses can be many different types. We will only handle system memory (MMIO)
        .filter(|base_address| base_address.address_space == AddressSpace::SystemMemory)
        .filter(|base_address| {
            base_address.bit_offset == 0 && base_address.bit_width.is_multiple_of(8)
        })
        .map(|base_address| {
            let stride_bytes = base_address.bit_width / 8;
            let memory = MEMORY.get().unwrap();
            let phys_start_address = base_address.address;
            let len = u64::from(stride_bytes) * 8;
            let start_frame = Frame::new(
                PhysAddr::new(phys_start_address).align_down(page_size.byte_len_u64()),
                page_size,
            )
            .unwrap();
            let n_pages = (phys_start_address + len).div_ceil(page_size.byte_len_u64())
                - phys_start_address / page_size.byte_len_u64();
            let mut physical_memory = memory.physical_memory.lock();
            let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
            let mut virtual_memory = memory.virtual_memory.lock();
            let mut allocated_pages = virtual_memory
                .allocate_contiguous_pages(page_size, n_pages)
                .unwrap();
            let start_page = Page::new(allocated_pages.start_addr(), page_size).unwrap();
            for i in 0..n_pages {
                let page = start_page.offset(i).unwrap();
                let frame = start_frame.offset(i).unwrap();
                let flags = ConfigurableFlags {
                    writable: true,
                    executable: false,
                    pat_memory_type: PatMemoryType::StrongUncacheable,
                };
                // Safety: the memory we are going to access is defined to be valid
                unsafe { allocated_pages.map_to(page, frame, flags, &mut frame_allocator) }
                    .unwrap();
            }
            let base_pointer = (start_page.start_addr()
                + phys_start_address % page_size.byte_len_u64())
            .as_mut_ptr();
            unsafe { UartWriter::new(MmioAddress::new(base_pointer, stride_bytes as usize), false) }
        })
    {
        // TODO: Make logger output to this UART
        log::info!("Replaced COM1 with MMIO UART from the SPCR ACPI table");
    }
}
```
At this point, we should be able to use the `Write` trait to write to the serial port `uart`

## Replacing the serial port used for logging
Currently, we are using a `uart_16550::port::SerialPort`. But if we use the SPCR, we will get a `UartWriter<MmioAddress>`. Both of these structs implement `Write`, but we need to somehow store *either* of them. That's where the `either` crate is useful:
```toml
either = { version = "1.15.0", default-features = false }
```
Then, we just need to make a few changes to `logger.rs`:
In `Inner`:
```rs
serial_port: Either<SerialPort, UartWriter<MmioAddress>>
```
And initialize it with:
```rs
Either::Left(unsafe { SerialPort::new(0x3F8) })
```
In the `init` function:
```rs
if let Either::Left(serial_port) = &mut inner.serial_port {
    serial_port.init();
} 
```

Next, we will make a function to replace the serial port:
```rs
pub fn replace_serial_port(serial_port: UartWriter<MmioAddress>) {
    LOGGER.inner.lock().serial_port = Either::Right(serial_port);
}
```
Now back in `spcr.rs`, all we have to do is replace the comment with
```rs
logger::replace_serial_port(uart);
```
This will result in all future log messages no longer sending data to COM1, and instead sending them to the MMIO UART.

## Putting it together
In `init_bsp`, replace the ACPI tables logging with:
```rs
let acpi_tables = acpi::parse(rsdp);
spcr::init(&acpi_tables);
```

## Trying it out
Here are the steps to trying it on a Chromebook:
- Plug in your debug board / cable. It should show up as a USB device on the computer that you are debugging your Chromebook with.
- Use a command such as `tio` to view output from `/dev/ttyUSB1`. If you are using `tio`, just run `tio /dev/ttyUSB1` (to exit, do `Ctrl` + `T` and then `Q`).
- Boot the OS on your Chromebook

Note that you will not see "Hello from BSP" because that will be unconditionally sent to COM1. You should see "Replaced COM1 with MMIO UART from the SPCR ACPI table". You should also see all "Hello from AP" messages, because we start running `entry_point_ap` *after* parsing the SPCR.
