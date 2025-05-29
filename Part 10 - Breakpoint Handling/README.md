# Handling interrupts and exceptions
An interrupt is when the CPU receives an external event. Like the word "interrupt", when the CPU receives the event, the CPU will interrupt your code and switch to executing some other code. The CPU sometimes switches stacks (by changing the `rsp` register), and basically "calls" the interrupt handler function with an interrupt stack frame. It is the interrupt handler function's responsibility to switch back to whatever the CPU was doing before.

An exception is when the code tries to do something invalid. For example, a page fault is when code tries to access an invalid memory address. A double fault is when there is an exception that happens as the CPU tries to execute an exception handler. A triple fault happens if there is an exception as the CPU tries to execute the double fault handler. When a triple fault happens, the computer immediately reboots. Similar to interrupts, the CPU jumps to an exception handler function.

As we write an OS, there **will** be exceptions because of some bug in our code. We'll define exception handlers for them, because if we don't, there will be a triple fault and that's hard to debug. We'll start by having a [breakpoint](https://wiki.osdev.org/Exceptions#Breakpoint) exception handler. Breakpoints aren't really errors, and this "exception" is convenient to check that our exception handlers are working.

There are three things we need to set up: The GDT, TSS, and IDT. The IDT contains the handler functions that should be executed on different kinds of exceptions and interrupts. The TSS contains pointers to stacks when the CPU switches stacks before executing a handler. The GDT in modern times basically just contains a pointer to the TSS. Each CPU will have its own GDT, TSS, and IDT. Create a file `gdt.rs`:
```rs
pub struct Gdt {
    gdt: GlobalDescriptorTable,
    kernel_code_selector: SegmentSelector,
    kernel_data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}
```
We create a `struct Gdt` which contains the actual GDT along with segment selectors (don't worry about them).

Let's add the TSS, GDT, and IDT to the `CpuLocalData`:
```rs
pub struct CpuLocalData {
    pub cpu: &'static Cpu,
    pub tss: OnceCell<TaskStateSegment>,
    pub gdt: OnceCell<Gdt>,
    pub idt: OnceCell<InterruptDescriptorTable>,
}
```
And make them initially `OnceCell::uninit()`.

Now we create an `init` function in `gdt.rs`:
```rs
/// # Safety
/// This function must be called exactly once
pub unsafe fn init() { }
```
Because the GDT requires a pointer to the TSS, we first initialize the TSS:
```rs
let local = get_local();
let tss = {
    local.tss.try_init_once(|| TaskStateSegment::new()).unwrap();
    local.tss.try_get().unwrap()
};
```
For now, we won't put anything in the TSS, and we'll have an empty TSS. Next, we create the GDT:
```rs
let gdt = {
    local
        .gdt
        .try_init_once(|| {
            let mut gdt = GlobalDescriptorTable::new();
            let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
            let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
            let tss_selector = gdt.append(Descriptor::tss_segment(tss));
            Gdt {
                gdt,
                kernel_code_selector,
                kernel_data_selector,
                tss_selector,
            }
        })
        .unwrap();
    local.gdt.try_get().unwrap()
};
```
Next, we load the GDT:
```rs
gdt.gdt.load();
```
We have to set some registers to specific values:
```rs
unsafe { CS::set_reg(gdt.kernel_code_selector) };
unsafe { SS::set_reg(gdt.kernel_data_selector) };
```
And we load the tss:
```rs
unsafe { load_tss(gdt.tss_selector) };
```
Note that we don't input the pointer to the TSS directly when loading the TSS. Instead, we input the TSS's segment selector in the now loaded GDT.

Next let's set up the IDT. Create `idt.rs`. First let's create our breakpoint handler function:
```rs
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    log::info!("Breakpoint! Stack frame: {:#?}", stack_frame);
}
```
When we specify `extern "x86-interrupt"`, Rust will handle restoring what the CPU was previously doing for us. It will also restore any registers that it changed. We need to add
```rs
#![feature(abi_x86_interrupt)]
```
to `main.rs` in order to use the `x86-interrupt` calling convention.

Next, we create a function to create and load the idt:
```rs
pub fn init() {
    let idt = &get_local().idt;
    let idt = {
        idt.try_init_once(|| {
            let mut idt = InterruptDescriptorTable::new();
            idt.breakpoint.set_handler_fn(breakpoint_handler);
            idt
        })
        .unwrap();
        idt.try_get().unwrap()
    };
    idt.load();
}
```
For now, we're only doing
```rs
idt.breakpoint.set_handler_fn(breakpoint_handler);
```
But later, we will add handlers for other exceptions and for interrupts.

Finally, let's call the functions at the bottom of `entry_point_from_limine` and `entry_point_from_limine_mp`:
```rs
unsafe { gdt::init() };
idt::init();
x86_64::instructions::interrupts::int3();
```
The `int3` instruction triggers a breakpoint.

Now when we run the OS, we'll see:
```
[BSP] INFO  Hello World!
[BSP] INFO  CPU Count: 2
[CPU 1] INFO  Hello from CPU 1
[CPU 0] INFO  Breakpoint! Stack frame: InterruptStackFrame {
    instruction_pointer: VirtAddr(
        0xffffffff80016141,
    ),
    code_segment: SegmentSelector {
        index: 1,
        rpl: Ring0,
    },
    cpu_flags: RFlags(
        SIGN_FLAG | 0x2,
    ),
    stack_pointer: VirtAddr(
        0xffff800003bc6e58,
    ),
    stack_segment: SegmentSelector {
        index: 2,
        rpl: Ring0,
    },
}
[CPU 1] INFO  Breakpoint! Stack frame: InterruptStackFrame {
    instruction_pointer: VirtAddr(
        0xffffffff80016141,
    ),
    code_segment: SegmentSelector {
        index: 1,
        rpl: Ring0,
    },
    cpu_flags: RFlags(
        SIGN_FLAG | PARITY_FLAG | 0x2,
    ),
    stack_pointer: VirtAddr(
        0xffff800002655f18,
    ),
    stack_segment: SegmentSelector {
        index: 2,
        rpl: Ring0,
    },
}
```
Now that we know breakpoint handling works, let's remove the `x86_64::instructions::interrupts::int3();`.

# Learn more
- https://os.phil-opp.com/cpu-exceptions/
- https://os.phil-opp.com/double-fault-exceptions/
- https://os.phil-opp.com/hardware-interrupts/
- https://wiki.osdev.org/Exceptions
- https://wiki.osdev.org/Global_Descriptor_Table
- https://wiki.osdev.org/Task_State_Segment
- https://wiki.osdev.org/Interrupt_Descriptor_Table
