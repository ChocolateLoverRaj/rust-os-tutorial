# Handling exceptions
Like I mentioned before, we **will** have exceptions, and if we can panic, log debug info, and use the debugger, we will have a much better debugging experience than if we let it triple fault.

## Triggering a page fault
Let's try purposely creating an exception:
```rs
unsafe {
    (0xABCDEF as *mut u8).read_volatile();
}
```
here we are purposely triggering a [page fault](https://wiki.osdev.org/Exceptions#Page_Fault). The address `0xABCDEF` is invalid, and we are reading from it. If you run QEMU now, it will triple fault, and QEMU will reboot the VM, causing an endless loop of rebooting and triple faulting. Let's do two things to make this easier to debug. Let's pass `--no-reboot`, which makes QEMU exit without rebooting in the event of a triple fault. And also, `-d int`, which makes QEMU print all interrupts and exceptions that happen. Now, when we run the VM again, we should see:
```
check_exception old: 0xffffffff new 0xe
   285: v=0e e=0000 i=0 cpl=0 IP=0008:ffffffff80007d43 pc=ffffffff80007d43 SP=0000:ffff800003be8e60 CR2=0000000000abcdef
RAX=0000000000abcdef RBX=0000000000000000 RCX=0000000000000000 RDX=3333333333333333
RSI=0000000000000001 RDI=0000000000abcdef RBP=0000000000000000 RSP=ffff800003be8e60
R8 =ffffffff80014800 R9 =8000000000000001 R10=ffffffff80016400 R11=00000000000010e0
R12=0000000000000000 R13=0000000000000000 R14=0000000000000000 R15=0000000000000000
RIP=ffffffff80007d43 RFL=00000082 [--S----] CPL=0 II=0 A20=1 SMM=0 HLT=0
ES =0000 0000000000000000 00000000 00000000
CS =0008 0000000000000000 ffffffff 00af9b00 DPL=0 CS64 [-RA]
SS =0000 0000000000000000 ffffffff 00c09300 DPL=0 DS   [-WA]
DS =0000 0000000000000000 00000000 00000000
FS =0030 0000000000000000 00000000 00009300 DPL=0 DS   [-WA]
GS =0030 ffffffff80019320 00000000 00009300 DPL=0 DS   [-WA]
LDT=0000 0000000000000000 00000000 00008200 DPL=0 LDT
TR =0010 ffffffff80019328 00000067 00008900 DPL=0 TSS64-avl
GDT=     ffffffff8001a3a0 0000001f
IDT=     ffffffff80019390 00000fff
CR0=80010011 CR2=0000000000abcdef CR3=0000000003bd8000 CR4=00000020
DR0=0000000000000000 DR1=0000000000000000 DR2=0000000000000000 DR3=0000000000000000
DR6=00000000ffff0ff0 DR7=0000000000000400
CCS=0000000000000078 CCD=ffff800003be8e58 CCO=ADDQ
EFER=0000000000000d00
```
That's a lot of information! Here are some key details:
```
check_exception old: 0xffffffff new 0xe
```
The `0xe` means that a page fault happened. You can reference [this table](https://wiki.osdev.org/Exceptions) to check the exception based on the code.

`IP=0008:ffffffff80007d43` means that `0xffffffff80007d43` is the pointer to the instruction that caused the page fault.

`CR2=0000000000abcdef` means that the address `0x0000000000abcdef` was accessed, which caused the page fault. This matches what we wrote in the Rust code.

Scrolling down, we can see `check_exception old: 0xe new 0xb`. `0xb` means a "Segment Not Present" fault occurred. Next, `check_exception old: 0x8 new 0xb`. The `0x8` indicates a double fault. It seems like the double fault caused another "segment not present" fault, which caused a triple fault.

## A double fault handler
Let's create a double fault handler:
```rs
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!(
        "Double Fault! Stack frame: {:#?}. Error code: {}.",
        stack_frame, error_code
    )
}
```
And then add it to our IDT
```rs
idt.double_fault.set_handler_fn(double_fault_handler);
```
Now our page fault will still trigger a double fault, but we'll handle the double fault:
```
[CPU 0] ERROR panicked at kernel/src/idt.rs:13:5:
Double Fault! Stack frame: InterruptStackFrame {
    instruction_pointer: VirtAddr(
        0xffffffff80007f33,
    ),
    code_segment: SegmentSelector {
        index: 1,
        rpl: Ring0,
    },
    cpu_flags: RFlags(
        SIGN_FLAG | 0x2,
    ),
    stack_pointer: VirtAddr(
        0xffff800003be8e60,
    ),
    stack_segment: SegmentSelector {
        index: 0,
        rpl: Ring0,
    },
}. Error code: 0.
```

## A page fault handler
Now let's add a page fault, as page faults will probably be the most common type of exception that happens:
```rs
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!(
        "Double Fault! Stack frame: {:#?}. Error code: {}.",
        stack_frame, error_code
    )
}
```
```rs
idt.page_fault.set_handler_fn(page_fault_handler);
```

Now we should see this:
```
Page fault! Stack frame: InterruptStackFrame {
    instruction_pointer: VirtAddr(
        0xffffffff80008133,
    ),
    code_segment: SegmentSelector {
        index: 1,
        rpl: Ring0,
    },
    cpu_flags: RFlags(
        RESUME_FLAG | SIGN_FLAG | 0x2,
    ),
    stack_pointer: VirtAddr(
        0xffff800003be8e60,
    ),
    stack_segment: SegmentSelector {
        index: 0,
        rpl: Ring0,
    },
}. Error code: PageFaultErrorCode(
    0x0,
).
```
If you want, you can add handler functions for other types of exceptions too.
