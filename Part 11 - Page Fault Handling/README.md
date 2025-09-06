# Handling page faults
Like I mentioned before, we **will** have exceptions, and if we can panic, log debug info, and use the debugger, we will have a much better debugging experience than if we let it triple fault.

## Triggering a page fault
Let's try purposely creating a page fault:
```rs
unsafe {
    (0xABCDEF as *mut u8).read_volatile();
}
```
here we are purposely triggering a [page fault](https://wiki.osdev.org/Exceptions#Page_Fault). The address `0xABCDEF` is invalid, and we are reading from it. If you run QEMU now, it will triple fault, and QEMU will reboot the VM, causing an endless loop of rebooting and triple faulting. Let's do two things to make this easier to debug. Let's pass `--no-reboot`, which makes QEMU exit without rebooting in the event of a triple fault. And also, `-d int`, which makes QEMU print all interrupts and exceptions that happen. Let's add `-d int` to our `tasks.json` for convenience. Now, when we run the VM again, we should see:
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

## A page fault handler
Let's define a page fault handler in our IDT. Let's create the page fault handler function in `idt.rs`. In a page fault, we can read the `Cr2` register to get the accessed address that caused the page fault.
```rs
extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let accessed_address = Cr2::read().unwrap();
    panic!(
        "Page fault! Stack frame: {stack_frame:#?}. Error code: {error_code:#?}. Accessed address: {accessed_address:?}."
    )
}
```
Then let's add the function to the IDT:
```rs
idt.page_fault.set_handler_fn(page_fault_handler);
```

Now we should see this:
```
[0] ERROR panicked at kernel/src/idt.rs:17:5:
Page fault! Stack frame: InterruptStackFrame {
    instruction_pointer: VirtAddr(
        0xffffffff8000e220,
    ),
    code_segment: SegmentSelector {
        index: 1,
        rpl: Ring0,
    },
    cpu_flags: RFlags(
        RESUME_FLAG | SIGN_FLAG | PARITY_FLAG | 0x2,
    ),
    stack_pointer: VirtAddr(
        0xffff800003ba4ec0,
    ),
    stack_segment: SegmentSelector {
        index: 2,
        rpl: Ring0,
    },
}. Error code: PageFaultErrorCode(
    0x0,
). Accessed address: VirtAddr(0xabcdef).
```

## Stack Overflows
Our kernel now catches and prints errors caused by accessing an invalid address. However, there is a common type of error that our kernel does not "catch" - a stack overflow. A stack overflow is when we run out of stack memory, causing the CPU to access memory that is outside of the stack. Often, what's called a "guard page" is placed at the end of the stack. A guard page is purposely unmapped memory, so that a page fault will be triggered when a stack overflow happens. However, Limine does not set up guard pages for our stacks. This means that if a stack overflow happens, the stack could overwrite other memory, causing all sorts of unexpected behavior, exceptions, and triple faults - all very annoying to debug. For this reason, we will set up a stack with a guard page in future parts.

## Learn More
- https://en.wikipedia.org/wiki/Stack_Overflow
- https://wiki.osdev.org/Stack
