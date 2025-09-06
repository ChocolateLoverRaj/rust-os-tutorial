# CPU Local Data
So far, we've used global variables, which are shared with all code running on every CPU, and variables inside functions (on the stack). In our kernel, we will need global variables that are unique to each CPU.

We don't know how many CPUs our kernel will run on at compile time, so we will need to allocate and initialize our CPU-specific global variables at run time. Create a file `cpu_local_data.rs`. Let's make a struct for keeping CPU-specific global data:
```rs
pub struct CpuLocalData {
    /// Similar to [Linux](https://elixir.bootlin.com/linux/v5.6.3/source/arch/x86/kernel/apic/apic.c#L2469), the we assign the BSP id `0`.
    /// For the APs, they will have an id based on their position in the CPUs array given from Limine.
    pub kernel_assigned_id: u32,
    #[allow(unused)]
    pub local_apic_id: u32,
}
```

We can keep the `CpuLocalData`s in an array. In Rust, we will use a boxed slice, which is basically an array allocated in run time. One question we have to answer is which CPU will have which index in the array? We can all this index in the array an ID. Limine gives us a list of CPUs, with a ACPI id and a local APIC id. However, we cannot use either of these ids as an index in an array, because these ids are not guaranteed to start with `0` and could have gaps. So we can make our own ids, which we will assign to CPUs. We can call this id `kernel_assigned_id`. In this tutorial, we will always give the BSP id `0`, to easily recognize the BSP. We will give the other CPUs an id based on their index in the array of CPUs that Limine gives us.

Create a file `cpu_local_data.rs`. Let's make a struct for keeping CPU-specific global data:
```rs
pub struct CpuLocalData {
    /// Similar to [Linux](https://elixir.bootlin.com/linux/v5.6.3/source/arch/x86/kernel/apic/apic.c#L2469), the we assign the BSP id `0`.
    /// For the APs, they will have an id based on their position in the CPUs array given from Limine.
    pub kernel_assigned_id: u32,
    #[allow(unused)]
    pub local_apic_id: u32,
}
```

For our convenience, create this helper function:
```rs
fn mp_response() -> &'static MpResponse {
    MP_REQUEST.get_response().expect("expected MP response")
}
```

Because we only know the number of CPUs at runtime, we cay use lazy initialization to initialize an array of CPU local data:
```rs
static CPU_LOCAL_DATA: Lazy<Box<[Once<CpuLocalData>]>> =
    Lazy::new(|| mp_response().cpus().iter().map(|_| Once::new()).collect());
```

We also need a way to store the kernel assigned id of the current CPU. For this, we need to store it somewhere that is unique to a CPU. We will use the GS.Base register for this. GS.Base is supposed to store a pointer, and this register is not shared between CPUs. We can store a pointer to `CpuLocalData` in `GS.Base`:
```rs
/// This function makes sure that we are writing a valid pointer to CPU local data to GsBase
fn write_gs_base(ptr: &'static CpuLocalData) {
    GsBase::write(VirtAddr::from_ptr(ptr));
}
```

Next let's create functions to initialize CPU local data for the current CPU, and store the pointer in GS.Base:
```rs
/// Initializes the item in [`CPU_LOCAL_DATA`] and GS.Base
fn init_cpu(kernel_assigned_id: u32, local_apic_id: u32) {
    write_gs_base(
        CPU_LOCAL_DATA[kernel_assigned_id as usize].call_once(|| CpuLocalData {
            kernel_assigned_id,
            local_apic_id,
        }),
    );
}

/// Initialize CPU local data for the BSP
///
/// # Safety
/// Must be called on the AP
pub unsafe fn init_bsp() {
    init_cpu(
        // We always assign id 0 to the BSP
        0,
        mp_response().bsp_lapic_id(),
    );
}

/// # Safety
/// The CPU must match the actual CPU that this function is called on
pub unsafe fn init_ap(cpu: &Cpu) {
    let local_apic_id = cpu.lapic_id;
    init_cpu(
        // We get use the position of the CPU in the array, not counting the BSP and adding 1 because id `0` is the BSP.
        mp_response()
            .cpus()
            .iter()
            .filter(|cpu| cpu.lapic_id != mp_response().bsp_lapic_id())
            .position(|cpu| cpu.lapic_id == local_apic_id)
            .expect("CPUs array should contain this AP") as u32
            + 1,
        local_apic_id,
    );
}
```
In `entry_point_from_limine`, after `memory::init`, add:
```rs
// Safety: We are calling this function on the BSP
unsafe {
    cpu_local_data::init_bsp();
}
```
And in `entry_point_from_limine_mp` add:
```rs
// Safety: We're actually calling the function on this CPU
unsafe { cpu_local_data::init_ap(cpu) };
```

Now, let's create two helper functions in `cpu_local_data.rs`:
```rs
pub fn cpus_count() -> usize {
    mp_response().cpus().len()
}

pub fn try_get_local() -> Option<&'static CpuLocalData> {
    let ptr = NonNull::new(GsBase::read().as_mut_ptr::<CpuLocalData>())?;
    // Safety: we only wrote to GsBase using `write_gs_base`, which ensures that the pointer is `&'static CpuLocalData`
    unsafe { Some(ptr.as_ref()) }
}
```

## Showing the CPU in our logger
It is useful to know which CPU logged what message. Let's prefix all of our log messages with the CPU id. Let's add `Color::Gray`, with
```rs
Color::Gray => &string.dimmed()
```
for the serial logger and
```rs
Color::Gray => Rgb888::new(128, 128, 128)
```
for the screen. Then in the `log` method, add this before printing the log level:
```rs
let cpu_id = try_get_local().map_or(0, |data| data.kernel_assigned_id);
let width = match cpus_count() {
    1 => 1,
    n => (n - 1).ilog(16) as usize + 1,
};
inner.write_with_color(Color::Gray, format_args!("[{cpu_id:0width$X}] "));
```
Here we print the id (in hex) of the CPU that logged the message. We adjust the number of digits in the CPU id to be the maximum number of digits needed.

We can even try running our CPU with a ton of CPUs now. Add the following QEMU flags:

For 300 CPUs:
```
--smp 300
```

To use a non-default QEMU machine which can support this many CPUs:
```
--machine q35
```

To enable X2APIC, which is needed for >255 CPUs:
```
--cpu qemu64,+x2apic
```

To increase the memory to 1 GiB instead of the default 128 MiB, since we need more memory for all those extra CPUs:
```
-m 1G
```

Now the output should look similar to this:
```
[000] INFO  Hello from BSP
[0C1] INFO  Hello from AP
[101] INFO  Hello from AP
[0BF] INFO  Hello from AP
[059] INFO  Hello from AP
[0A0] INFO  Hello from AP
```
