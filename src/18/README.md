# Local APIC
Each CPU has its own local APIC (Advanced Programmable Interrupt Controller), which is a thing that sends interrupts the CPU. Local APICs can send interrupts to the Local APICs of other CPUs, which are called inter-processor interrupt, or IPIs. Local APICs themselves can receive interrupts from I/O APICs and then forward those interrupts to their CPU. A computer with APIC has to have at least 1 I/O APIC, and the I/O APIC can route interrupts to a local APIC. Most computers only have 1 I/O APIC, but technically, they can have more.

In this part, we will configure and receive interrupts from the local APIC.

## APIC crate
We will use this crate:
```toml
x2apic = { git = "https://github.com/ChocolateLoverRaj/x2apic-rs", version = "0.5.0" }
```
You may be wondering why it has an "x2" before APIC. APIC has 3 "versions": APIC (super old, we will not bother supporting), xAPIC (which QEMU has by default), and x2APIC (which is what modern computers have and). The `x2apic` crate works with both xAPIC and x2APIC.

## Mapping the Local APIC if needed
Create a file `apic.rs`:
```rs
#[derive(Debug)]
pub enum LocalApicAccess {
    /// No MMIO needed because x2apic uses register based configuration
    RegisterBased,
    /// The pointer to the mapped Local APIC
    Mmio(VirtAddr),
}

pub static LOCAL_APIC_ACCESS: Once<LocalApicAccess> = Once::new();

/// Maps the Local APIC memory if needed, and initializes LOCAL_APIC_ACCESS
pub fn init_bsp(acpi_tables: &AcpiTables<impl acpi::Handler>) {
    let apic = match InterruptModel::new(&acpi_tables).unwrap().0 {
        InterruptModel::Apic(apic) => apic,
        interrupt_model => panic!("Unknown interrupt model: {:#?}", interrupt_model),
    };
    LOCAL_APIC_ACCESS.call_once(|| {
        if cpu_has_x2apic() {
            LocalApicAccess::RegisterBased
        } else {
            let page_size = PageSize::_4KiB;
            let frame = Frame::new(PhysAddr::new(apic.local_apic_address), page_size).unwrap();
            // Local APIC is always exactly 4 KiB, aligned to 4 KiB
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
            let mut virtual_memory = memory.virtual_memory.lock();
            let page = virtual_memory
                .allocate_contiguous_pages(page_size, NonZero::new(1).unwrap())
                .unwrap();
            let flags = ConfigurableFlags {
                writable: true,
                executable: false,
                // We use strong uncacheable memory type, because reads and writes have side effects
                pat_memory_type: PatMemoryType::StrongUncacheable,
            };
            // Safety: We map to the correct page for the Local APIC
            unsafe {
                virtual_memory
                    .l4_mut()
                    .map_page(page, frame, flags, &mut frame_allocator)
            }
            .unwrap();
            LocalApicAccess::Mmio(page.start_addr())
        }
    });
}
```
and then in `main.rs`, after `spcr::init`, add:
```rs
local_apic::map_if_needed(&acpi_tables);
```

## Initializing the local APIC
We will be getting a `LocalApic` struct from the `x2apic` crate. We will store it in CPU local data. One issue is that `LocalApic` is `!Send` and `!Sync`, so Rust will not allow us to put `LocalApic` in CPU local data. The reason that [`LocalApic` is `!Send` and `!Sync`](https://github.com/kwzhao/x2apic-rs/commit/38bb9d5f88964c00f65b31e447ed95af825933b5) is that it is not safe to send across *CPUs*. We are not sending it across CPUs, so we can safely ignore the `!Send` and `!Sync`. To ignore these, we will use the `force-send-sync` crate:
```toml
force-send-sync = { git = "https://github.com/ChocolateLoverRaj/force-send-sync", branch = "no_std", version = "1.1.0" }
```
Then, in `CpuLocalData`, add:
```rs
pub local_apic: Once<spin::Mutex<SendSync<LocalApic>>>,
```
Before we can build a `LocalApic` with `LocalApicBuilder`, we need to have a spurious, error, and timer interrupt vector. In x86_64, you can configure up to 256 different interrupts for the CPU to handle. Each interrupt index is called an *interrupt vector* (for some reason). Basically, we have to tell the local APIC, "if a spurious interrupt happens, trigger the CPUs interrupt at this interrupt index". Like the exception handlers, we configure the handler functions for the interrupt vectors in the IDT. The first 32 interrupts are for exceptions and reserved. After that, we can decide how we'll use the other interrupt vectors (up to index 255). To define which interrupt vectors are used for what, let's create an enum in a new file `interrupt_vector.rs`:
```rs
use num_enum::IntoPrimitive;

#[derive(Debug, IntoPrimitive)]
#[repr(u8)]
pub enum InterruptVector {
    LocalApicSpurious = 0x20,
    LocalApicTimer,
    LocalApicError,
}
```
Now, back in `apic.rs`, let's add a function that will get run on every CPU:
```rs
/// This function needs to be called on all CPUs.
/// [`init_bsp`] must be called first.
pub fn init_local_apic() {
    get_local().local_apic.call_once(|| {
        spin::Mutex::new({
            let local_apic = {
                let mut builder = LocalApicBuilder::new();
                // We only need to use `set_xapic_base` if x2APIC is not supported
                if let LocalApicAccess::Mmio(address) = LOCAL_APIC_ACCESS.get().unwrap() {
                    builder.set_xapic_base(address.as_u64());
                }
                builder.spurious_vector(u8::from(InterruptVector::LocalApicSpurious).into());
                builder.error_vector(u8::from(InterruptVector::LocalApicError).into());
                builder.timer_vector(u8::from(InterruptVector::LocalApicTimer).into());
                let mut local_apic = builder.build().unwrap();
                // Safety: We are ready to handle interrupts (and interrupts are disabled anyways)
                unsafe { local_apic.enable() };
                // Safety: We don't need the timer to be on
                unsafe { local_apic.disable_timer() };
                local_apic
            };
            // Safety: The only reason why LocalApic is marked as !Send and !Sync is because it cannot be accessed across CPUs. We are only accessing it from this CPU.
            unsafe { SendSync::new(local_apic) }
        })
    });
}
```
Then in `main.rs` add:
```rs
local_apic::init();
```
After `apic::init_bsp` in `init_bsp`, and after `idt::init()` in `init_ap`.

## Testing timer interrupts
To test if our code successfully set up interrupt handlers, let's try receiving timer interrupts. In `idt.rs`, add:
```rs
extern "x86-interrupt" fn apic_timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    log::info!("Received APIC timer interrupt");
    // We must notify the local APIC that it's the end of interrupt, otherwise we won't receive any more interrupts from it
    let mut local_apic = get_local().local_apic.get().unwrap().try_lock().unwrap();
    // Safety: We are done with an interrupt triggered by the local APIC
    unsafe { local_apic.end_of_interrupt() };
}
```
And then in the init function, similar to how we define exception handlers, we'll define an interrupt handler, but using the interrupt index:
```rs
idt[u8::from(InterruptVector::LocalApicTimer)].set_handler_fn(apic_timer_interrupt_handler);
```
Note that we did not define handlers spurious and error interrupt vectors. So if one of those does happen, it will result in a double fault. But it shouldn't happen.

Then, to test out that the timer interrupt is working, temporarily add this to the BSP and AP code (after initializing the local APIC):
```rs
// Remember to not hold the lock to the local APIC before enabling interrupts
{
    let mut local_apic = get_local().local_apic.get().unwrap().lock();
    unsafe {
        local_apic.set_timer_divide(x2apic::lapic::TimerDivide::Div128);
        local_apic.enable_timer();
    };
}
x86_64::instructions::interrupts::enable();
```
You should see all of the CPUs receive a timer interrupt:
```rs
[CPU 0] INFO  Received APIC timer interrupt
[CPU 4] INFO  Received APIC timer interrupt
[CPU 5] INFO  Received APIC timer interrupt
[CPU 1] INFO  Received APIC timer interrupt
[CPU 6] INFO  Received APIC timer interrupt
[CPU 2] INFO  Received APIC timer interrupt
[CPU 3] INFO  Received APIC timer interrupt
[CPU 7] INFO  Received APIC timer interrupt
```
and then, after some time, all CPUs will receive another timer interrupt, and they will continue to periodically receive them. The time between timer interrupts varies between computers. In qemu, it is a very short duration. On Jinlon, it is very long. On the Lenovo Z560, it is more often than Jinlon, but much less often than qemu.

# Learn More
- <https://wiki.osdev.org/APIC>
- <https://wiki.osdev.org/APIC_Timer>
- <https://en.wikipedia.org/wiki/Advanced_Programmable_Interrupt_Controller>
