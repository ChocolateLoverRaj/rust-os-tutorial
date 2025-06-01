# What CPU am I running on?
## CPU-specific global variables
In `entry_point_from_limine`, we know we are on the BSP (which stands for boot strap processor). In `entry_point_from_limine_mp`, we are given the `&Cpu` input, which tells us what CPU this function is running on. But there will be situations where we don't have an input which tells us our current CPU, such as in the logger. We need a way of saving the current CPU's index or ID. But we can't use a single global variable, since every CPU has a different ID. We'll store CPU-specific global variables in a `BTreeMap`, where the key is the Local APIC id (don't worry about what that is right now, you just need to know that this is a unique id associated with every CPU, and Limine provides a `bsp_lapic_id` method to get the Local APIC id of the BSP). Create a file `cpu_local_data.rs`. Let's make a struct for keeping CPU-specific global data:
```rs
pub struct CpuLocalData {
    pub cpu: &'static Cpu,
}
```
For now, we can just have a reference to the CPU. Later, we'll add more data.

We can't just use a `BTreeMap` as a global variable directly, since we'll need to initialize (which involves mutating) it at runtime. We could use a mutex for this, but there is a better data type. We'll use `spin::Once`. It's useful for global variables that will be initialized in run time and then never modified after that.
```rs
static CPU_LOCAL_DATA: Once<BTreeMap<u32, Box<CpuLocalData>>> = Once::new();
```
We use a `Box` to avoid stack overflows. As we add more members to `CpuLocalData`, it can get large and cause stack overflows it we move it around a lot. 

Then let's create a function to initialize them:
```rs
pub fn init(mp_response: &'static MpResponse) {
    CPU_LOCAL_DATA.call_once(|| {
        mp_response
            .cpus()
            .iter()
            .map(|cpu| (cpu.lapic_id, Box::new(CpuLocalData { cpu })))
            .collect()
    });
}
```
Here we can use `collect` to conveniently create the `BTreeMap` from the existing iterator, because [`BTreeMap` implements `FromIterator`](https://doc.rust-lang.org/std/collections/struct.BTreeMap.html#impl-FromIterator%3C(K,+V)%3E-for-BTreeMap%3CK,+V%3E).

Let's initialize the CPU local data in the entry function, before we set the `goto_address` for the other CPUs:
```rs
cpu_local_data::init(mp_response);
```
Now every CPU has it's own `CpuLocalData`. But we still need to know what the index is in the slice for the current CPU. For that, we'll use the `GsBase` register. This register, like other registers, is not shared between CPUs. It's used to store a pointer. So let's set the `GsBase` register to point to our specific CPU's data:
```rs
/// This function makes sure that we are writing a valid pointer to CPU local data to GsBase
fn write_gs_base(ptr: &'static CpuLocalData) {
    GsBase::write(VirtAddr::from_ptr(ptr));
}

/// # Safety
/// The Local APIC id must match the actual CPU that this function is called on
pub unsafe fn init_cpu(local_apic_id: u32) {
    write_gs_base(CPU_LOCAL_DATA.get().unwrap().get(&local_apic_id).unwrap());
}
```
Converting references to and from raw pointers without mistakes can be really tricky, even in Rust. The `fn write_gs_base(ptr: &'static CpuLocalData) {}` will make it so we don't accidentally store a pointer to the *`Box<CpuLocalData>`* istead of a pointer to `CpuLocalData`. 

Right away, let's call this function to initialize the BSP's CPU local data in `entry_point_from_limine`:
```rs
// Safety: We are calling this function on the BSP
unsafe {
    init_cpu(mp_response.bsp_lapic_id());
}
```

Then let's also initialize it for the other CPUs, in `entry_point_from_limine_mp`:
```rs
// Safety: We're inputting the correct CPU local APIC id
unsafe { init_cpu(cpu.lapic_id) };
```

Let's also create wrappers for accessing the CPU local data:
```rs
pub fn get_local() -> &'static CpuLocalData {
    try_get_local().unwrap()
}

pub fn try_get_local() -> Option<&'static CpuLocalData> {
    let ptr = GsBase::read().as_ptr::<CpuLocalData>();
    // Safety: we only wrote to GsBase using `write_gs_base`, which ensures that the pointer is `&'static CpuLocalData`
    unsafe { ptr.as_ref() }
}
```
Note that we check that if the CPU local data is not initialized, `GsBase` will be `0` (because Limine sets it to zero), and we return `None` since it's a null pointer.

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
if let Some(cpu_local_data) = try_get_local() {
    let cpu_id = cpu_local_data.cpu.id;
    inner.write_with_color(Color::Gray, format_args!("[CPU {cpu_id}] "));
} else {
    inner.write_with_color(Color::Gray, "[BSP] ");
};
```
Because we might want to log messages before initializing the `CPU_LOCAL_DATA`, we just print "BSP" instead of the CPU id if the cpu local data is not initialized. Now our kernel should log this:
```
[BSP] INFO  Hello World!
[BSP] INFO  CPU Count: 2
[CPU 0] INFO  Hello from BSP
[CPU 1] INFO  Hello from CPU 1
```
