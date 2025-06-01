# Serial port on Chromebooks
It can be really hard to debug when things don't work on real devices. We can log things to the screen, but we are limited to what we can see on the screen. We already log to COM1, but we can only see the it in a virtual machine. Fortunately, some computers (I know Chromebooks can, I don't know any others) do have a serial port. There is an ACPI table, [SPCR](https://learn.microsoft.com/en-us/windows-hardware/drivers/bringup/serial-port-console-redirection-table), which defines how to access a serial port. 

## Chromebooks
See https://docs.mrchromebox.tech/docs/support/debugging.html#suzyqable-debug-cable. Basically, you need to be running MrChromebox UEFI firmware with the coreboot flags `CONSOLE_SERIAL=y` and `EDK2_SERIAL_SUPPORT=y`, and have a SuzyQable Debug Cable. You can [make a debug cable](https://chromium.googlesource.com/chromiumos/third_party/hdctools/+/main/docs/ccd.md#SuzyQ-SuzyQable) or [buy one](https://github.com/ChocolateLoverRaj/gsc-debug-board/).

## Other computers
Since SPCR is a standard ACPI table, there is a chance that the code in this tutorial will just work. But you might have to modify it depending on whats in the SPCR table.

## Parsing the SPCR
Create a file `spcr.rs`. Again, we will be mapping page tables, so we're going to ake a generic function that takes `PageSize`. We will also add the `uart` crate, which let's us set the correct baud rate for Chromebooks:
```toml
uart = { git = "https://github.com/ChocolateLoverRaj/uart", branch = "send-sync" }
```
```rs
fn init_with_page_size<S: PageSize + Debug>(acpi_tables: &AcpiTables<impl AcpiHandler>)
where
    for<'a> OffsetPageTable<'a>: Mapper<S>,
{
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
            let phys_end_address_inclusive = phys_start_address + (u64::from(stride_bytes) * 8 - 1);
            let start_frame = PhysFrame::<S>::containing_address(PhysAddr::new(phys_start_address));
            let end_frame =
                PhysFrame::containing_address(PhysAddr::new(phys_end_address_inclusive));
            let mut physical_memory = memory.physical_memory.lock();
            let mut virtual_memory = memory.virtual_memory.lock();
            let n_pages = start_frame - end_frame + 1;
            let mut allocated_pages = virtual_memory.allocate_contiguous_pages(n_pages).unwrap();
            let start_page = *allocated_pages.range().start();
            for i in 0..n_pages {
                let frame = start_frame + i;
                let page = start_page + i;
                // Safety: the memory we are going to access is defined to be valid
                unsafe {
                    allocated_pages.map_to(
                        page,
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::NO_EXECUTE
                            | PageTableFlags::NO_CACHE,
                        physical_memory.deref_mut(),
                    )
                };
            }
            let base_pointer =
                (start_page.start_address() + phys_start_address % S::SIZE).as_mut_ptr();
            unsafe { UartWriter::new(MmioAddress::new(base_pointer, stride_bytes as usize), false) }
        })
    {
        // At this point we can write to the uart!
    }
}

/// Checks for SPCR, and sets logger to log through SPCR instead of COM1 accordingly
pub fn init(acpi_tables: &AcpiTables<impl AcpiHandler>) {
    if CpuId::new()
        .get_extended_processor_and_feature_identifiers()
        .unwrap()
        .has_1gib_pages()
    {
        init_with_page_size::<Size1GiB>(acpi_tables)
    } else {
        init_with_page_size::<Size2MiB>(acpi_tables)
    }
}
```
At this point, we should be able to use the `Write` trait to write to the serial port `uart`. However, if we view the logs through a program like `tio`, new lines will not look good because `tio` expects CRLF. So we need to make sure all `\n`s have a `\r` before them when writing to the serial port.

## Adding `\r`s
Create a file `writer_with_cr.rs`:
```rs
use core::fmt::Write;

use unicode_segmentation::UnicodeSegmentation;

/// A writer that writes to a writer but replaces `\n` with `\r\n`
pub struct WriterWithCr<T> {
    writer: T,
}

impl<T> WriterWithCr<T> {
    pub const fn new(writer: T) -> Self {
        Self { writer }
    }
}

impl<T: Write> Write for WriterWithCr<T> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.graphemes(true) {
            match c {
                "\n" => self.writer.write_str("\r\n")?,
                s => self.writer.write_str(s)?,
            }
        }
        Ok(())
    }
}
```
This way, we can log things with just a `\n`, and use `WriterWithCr` to automatically make sure it gets written as `\r\n`.

## Making the serial port replaceablee in the logge
Let's change `serial_port` in the `Inner` struct in `logger.rs` to be able to store anything that implements `Write`. Normally, we can just use `Box<dyn Write + Send + Sync>` for this. However, when we initialize the logger, the global allocator is not available, so we cannot create a `Box`. Let's create an enum that will let us store either a `SerialPort` or a `Box`:
```rs
pub enum AnyWriter {
    Com1(SerialPort),
    Boxed(Box<dyn Write + Send + Sync>),
}

impl Deref for AnyWriter {
    type Target = dyn Write;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Com1(r) => r,
            Self::Boxed(b) => b.as_ref(),
        }
    }
}

impl DerefMut for AnyWriter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Com1(r) => r,
            Self::Boxed(b) => b.as_mut(),
        }
    }
}
```
Then, let's change `serial_port` in `Inner`:
```rs
serial_port: Option<AnyWriter>,
```
A benefit of using `Option` is that if we detect that COM1 is not available as a serial port and there is no other serial port available, we can disable logging to a serial port entirely.

Let's set the initial value of the logger to have `None`:
```rs
static LOGGER: KernelLogger = KernelLogger {
    inner: spin::Mutex::new(Inner {
        serial_port: None,
        display: None,
    }),
};
```
And then set the serial port to COM1 initially in our init function:
```rs
inner.serial_port = Some(AnyWriter::Com1({
    // Safety: this is the only code that is accessing COM1
    let mut serial_port = unsafe { SerialPort::new(0x3F8) };
    serial_port.init();
    serial_port
}));
```
Then let's add a function which will let us replace the serial port from `spcr.rs`:
```rs
/// Replaces the serial logger, setting it to `None` if specified
pub fn replace_serial_logger(new_serial_logger: Option<AnyWriter>) {
    LOGGER.inner.lock().serial_port = new_serial_logger;
}
```

## Replacing the serial port
In `spcr.rs`, add:
```rs
logger::replace_serial_logger(Some(AnyWriter::Boxed(Box::new(uart))));
```

## Calling `spcr::init`
In `main.rs`, after getting the ACPI tables, before logging them, add:
```rs
spcr::init(&acpi_tables);
```

## Trying it out
Here are the steps to trying it on a Chromebook:
- Plug in your debug board / cable. It should show up as a USB device on the computer that you are debugging your Chromebook with.
- Use a command such as `tio` to view output from `/dev/ttyUSB1`. If you are using `tio`, just run `tio /dev/ttyUSB1` (to exit, do `Ctrl` + `T` and then `Q`).
- Boot the OS on your Chromebook

Here is the output on my Jinlon Chromebook:
```
[BSP] INFO  Hello World!
[BSP] INFO  ACPI Tables: ["FACP", "APIC", "HPET", "WAET", "BGRT"]
[BSP] INFO  CPU Count: 8
[CPU 2] INFO  Hello from CPU 2
[CPU 7] INFO  Hello from CPU 7
[CPU 6] INFO  Hello from CPU 6
[CPU 5] INFO  Hello from CPU 5
[CPU 4] INFO  Hello from CPU 4
[CPU 1] INFO  Hello from CPU 1
[CPU 3] INFO  Hello from CPU 3
```
