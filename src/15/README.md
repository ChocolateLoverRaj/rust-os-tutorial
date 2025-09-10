There are [many different exceptions](https://wiki.osdev.org/Exceptions#Segment_Not_Present) that could happen if there is a bug in our code. We already have a page fault handler. We *could* define a handler for every single possible exception. But to keep things simple, we'll just define a double fault handler. A double fault will trigger if any of the other exception handlers are not present. Unless we use `-d int`, we won't know what fault led to a double fault, but that's okay.

Let's define a double fault handler:
```rs
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!("Double Fault! Stack frame: {stack_frame:#?}. Error code: {error_code}.")
}
```
And add it to our IDT:
```rs
unsafe {
    idt.double_fault
        .set_handler_fn(double_fault_handler)
        .set_stack_index(u8::from(IstStackIndexes::Exception).into())
};
```

Again, we will use the same stack that we use for handling page faults, in case the stack pointer was not pointing to a valid stack when the double fault happened.

Now let's purposely cause a double fault to test it:
```rs
unsafe { core::arch::asm!("ud2") };
```
This code does a `ud2` instruction, which causes an invalid opcode exception. Since we did not define an exception handler for an invalid opcode exception, it leads to a double fault.
