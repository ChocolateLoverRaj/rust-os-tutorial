# What CPU am I running on?
## CPU-specific global variables
In our `entry_point_from_limine_mp`, we know what CPU we are running on because of the `&Cpu` input. But there will be situations where we don't have an input which tells us our current CPU, such as in the logger. We need a way of saving the current CPU's index or ID. But we can't use a single global variable, since every CPU has a different ID. We'll make a slice for CPU-specific global variables. Create a file `cpu_local_data.rs`. Let's make a struct for keeping CPU-specific global data:
```rs
pub struct CpuLocalData {
    pub cpu: &'static Cpu,
}
```
For now, we can just have a reference to the CPU. Later, we'll add more data.

We'll use a `Box<[CpuLocalData]>` to store all of the data for all CPUs. We'll use a boxed slice and not an array because at compile time we don't know how many CPUs we'll be booted with. There might be 1 CPU, or 255! We can't create the `Box` initially. We have to create it later. We could use a mutex for this, but there is a better data type. We'll use `OnceCell` from the `conquer-once` crate. It's useful for global variables that will be initialized in run time and then never modified after that.
```toml
conquer-once = { version = "0.4.0", default-features = false }
```
```rs
static CPU_LOCAL_DATA: OnceCell<Box<[CpuLocalData]>> = OnceCell::uninit();
```
Then let's create a function to initialize them:
```rs
pub fn init(mp_response: &'static MpResponse) {
    CPU_LOCAL_DATA
        .try_init_once(|| {
            mp_response
                .cpus()
                .iter()
                .map(|cpu| CpuLocalData { cpu })
                .collect()
        })
        .unwrap();
}
```
And let's call it from our entry function, before we set the `goto_address` for the other CPUs:
```rs
cpu_local_data::init(mp_response);
```
Now every CPU has it's own `CpuLocalData`. But we still need to know what the index is in the slice for the current CPU. For that, we'll use the `GsBase` register. This register, like other registers, is not shared between CPUs. It's used to store a pointer. So let's set the `GsBase` register to point to our specific CPU's data:
```rs
/// # Safety
/// The `local_cpu` must actually be the CPU that this functin is called on
pub unsafe fn init_cpu(mp_response: &MpResponse, local_cpu: &Cpu) {
    GsBase::write(VirtAddr::from_ptr(
        &CPU_LOCAL_DATA.try_get().unwrap()[mp_response
            .cpus()
            .iter()
            .position(|cpu| cpu.id == local_cpu.id)
            .unwrap()],
    ));
}
```
Note that we are using the order of CPU's from Limine's MP response as the order for our slice.

Then the first thing we'll do in `entry_point_from_limine_mp` is:
```rs
// Safety: We're inputting the correct CPU
unsafe { init_cpu(MP_REQUEST.get_response().unwrap(), cpu) };
```
And let's also set it for our initial CPU, also called the BSP, which stands for Boot Strap Processor:
```rs
unsafe {
    init_cpu(
        mp_response,
        mp_response
            .cpus()
            .iter()
            .find(|cpu| cpu.lapic_id == mp_response.bsp_lapic_id())
            .unwrap(),
    );
}
```
Let's also create wrappers for accessing the CPU local data:
```rs
pub fn get_local() -> &'static CpuLocalData {
    assert!(CPU_LOCAL_DATA.is_initialized());
    unsafe { GsBase::read().as_ptr::<CpuLocalData>().as_ref().unwrap() }
}

pub fn try_get_local() -> Option<&'static CpuLocalData> {
    if CPU_LOCAL_DATA.is_initialized() {
        unsafe { Some(GsBase::read().as_ptr::<CpuLocalData>().as_ref().unwrap()) }
    } else {
        None
    }
}
```
Note that we check that `CPU_LOCAL_DATA` is initialized, because if it's not initialized, then `GsBase` isn't loaded with the right pointer either.

## Showing the CPU in our logger
It is useful to know which CPU logged what message. Let's prefix all of our log messages with the CPU id. Let's replace the `writeln!` in our logger with this:
```rs
if let Some(cpu_local_data) = try_get_local() {
    let cpu_id = cpu_local_data.cpu.id;
    write!(serial_port, "{}", format_args!("[CPU {}]", cpu_id).dimmed()).unwrap();
} else {
    write!(serial_port, "{}", "[BSP]".dimmed()).unwrap();
}
let args = record.args();
writeln!(serial_port, " {:5} {}", level, args).unwrap();
```
Because we might want to log messages before initializing the `CPU_LOCAL_DATA`, we just print "BSP" instead of the CPU id. Now our kernel should log this:
```
[BSP] INFO  Hello World!
[BSP] INFO  CPU Count: 2
[CPU 0] INFO  Hello from BSP
[CPU 1] INFO  Hello from CPU 1
```
