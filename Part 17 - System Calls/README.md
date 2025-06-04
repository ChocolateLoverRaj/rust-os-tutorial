# What is a syscall?
By default, code running in user mode can't do anything useful. It can't draw to the screen, it can't even log messages. Only the kernel has permission to do those things. So how do we give certain permissions to a user mode program? With syscalls (short for system calls). 

Doing a syscall is similar to calling a function, except the function is a function in the kernel. In operating systems, there are different "functions" that the kernel provides to programs, and these functions are also called syscalls. Let's imagine a syscall which we add to our OS called `SAY_HI`. This is how it would work:
- The user mode program uses the `syscall` instruction, inputting some data to the kernel indicating that it wants to call the `SAY_HI` syscall.
- The CPU switches to kernel mode and jumps to the syscall handler
- The kernel parses and validates the syscall input, logs something like "user mode program says hi", and then uses `sysretq`
- The CPU switches to user mode and processes the next instruction after `syscall` in the user mode program

# Passing input and output data
The registers `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`, and `rax` can be set by the user mode program before doing the `syscall` instruction. That's `7` x `u64`. The kernel can then modify those registers before switching back to user space, so those registers can be used as outputs too. So effectively, we can input and output `[u64; 7]` (56 bytes) through registers (which are much faster and simpler than passing pointers).

In the user mode program, add
```rs
/// # Safety
/// The inputs must be valid. Invalid inputs can lead to undefined behavior or the program being terminated.
pub unsafe fn raw_syscall(inputs_and_ouputs: &mut [u64; 7]) {
    unsafe {
        asm!("\
            syscall
            ",
            inlateout("rdi") inputs_and_ouputs[0],
            inlateout("rsi") inputs_and_ouputs[1],
            inlateout("rdx") inputs_and_ouputs[2],
            inlateout("r10") inputs_and_ouputs[3],
            inlateout("r8") inputs_and_ouputs[4],
            inlateout("r9") inputs_and_ouputs[5],
            inlateout("rax") inputs_and_ouputs[6],
        );
    }
}
```
And then in `entry_point`, before our loop, let's try doing a syscall:
```rs
let mut inputs_and_outputs = [10, 20, 30, 40, 50, 60, 70];
unsafe { raw_syscall(&mut inputs_and_outputs) };
```

# Handling the syscall
We can't just set a Rust function for the syscall handler. First, we need to switch the stack pointer to a known good stack that only our kernel can access. We'll have to write our syscall handler in assembly up to the point where we can safely call a Rust function. The reason why we have to switch the stack pointer is because if we don't:
- There could be a stack overflow
- The user mode program could purposely cause a page fault by setting `rsp` to something unmapped
- The user mode could corrupt the kernel's memory
- The user mode program could just not mess with `rsp` when doing the syscall and then access the contents of the kernel's old stack after the syscall. This could possibly leak private data from the kernel to the user mode program.

We'll use a `naked` function to write the syscall handler in assembly while still integrating with Rust.
```rs
use core::arch::naked_asm;

#[unsafe(naked)]
unsafe extern "sysv64" fn raw_syscall_handler() -> ! {
    naked_asm!(
        "
            // assembly goes here
        "
    )
}
```
Before we do anything instruction involving the stack, such as `push`, `pop`, or `call`, we need to switch stacks. We can load the syscall handler's stack pointer from the CPU local data, and reference `GsBase` in our assembly code. In `CpuLocalData`, add:
```rs
pub syscall_handler_stack_pointer: SyncUnsafeCell<u64>,
pub user_mode_stack_pointer: SyncUnsafeCell<u64>,
```
And we can set it to `Default::default`.

TODO: Boxed stack for syscall handler, initialize `syscall_handler_stack_pointer`

We'll have to restore the current value of `rsp` later when returning from the syscall, so let's store it:
```rs
naked_asm!(
    "
        // Save the user mode stack pointer
        mov gs:[{user_mode_stack_pointer_offset}], rsp
    ",
    user_mode_stack_pointer_offset = const offset_of!(CpuLocalData, user_mode_stack_pointer),
)
```
Here are writing to a memory location specified by the value of `GsBase` + an offset, and we use the [`offset_of!`](https://doc.rust-lang.org/nightly/core/mem/macro.offset_of.html) macro to get the offset.

Similarly, let's set `rsp` to the kernel stack pointer:
```
// Switch to the kernel stack pointer
mov rsp, gs:[{syscall_handler_stack_pointer_offset}]
```
Again, using `offset_of!`. Now we successfully switched stacks!

We are almost ready to call a Rust function. We must pass some input to the Rust function. The Rust function needs:
- The 7 input registers (`rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`, `rax`)
- The pointer to the instruction that we should return to when returning from the syscall (currently stored in `rcx`)
- The value of the RFLAGS (currently stored in `r11`)

| Calling convention                                                                   | `input[0]` | `input[1]` | `input[2]` | `input[3]` | `input[4]` | `input[5]` | `input[6]` | Additional inputs              |
|--------------------------------------------------------------------------------------|------------|------------|------------|------------|------------|------------|------------|--------------------------------|
| Our `syscall` (we decide the order and usage)                                        | `rdi`      | `rsi`      | `rdx`      | `r10`      | `r8`       | `r9`       | `rax`      | N/A                            |
| [`sysv64`](https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI) | `rdi`      | `rsi`      | `rdx`      | `rcx`      | `r8`       | `r9`       | N/A        | On the stack, in reverse order |

As you can see, we can directly pass `rdi`, `rsi`, `rdx`, `r8`, and `r9` to our Rust function without modifying those registers. For `input[4]`, we can set `rcx` to the value of `r10`. We can pass `rax` as an additional input on the stack. We can also pass `rcx` and `r11` as additional inputs on the stack. This is how to do it in assembly, keeping in mind that additional arguments go on the stack **in reverse order**:
```
// This is input[8]
// Make sure to save `rcx` before modifying it
push rcx
// This is input[7]
push r11
// This is input[6]
push rax
// Convert `syscall`s `r10` input to `sysv64`s `rcx` input
mov rcx, r10
```
And we can specify our Rust function to match what the assembly calls it with:
```rs
unsafe extern "sysv64" fn syscall_handler(
    input0: u64,
    input1: u64,
    input2: u64,
    input3: u64,
    input4: u64,
    input5: u64,
    input6: u64,
    rflags: u64,
    return_instruction_pointer: u64,
) -> ! {
    todo!()
}
```
Now, from our assembly function, we can call our Rust function:
```
call {syscall_handler}
```
and we can do `syscall_handler = sym syscall_handler` to make our assembly function reference the pointer to our Rust function.

At this point, we can run the code and our inputs should be `[10, 20, 30, 40, 50, 60, 70]`.

## Returning from the syscall
To make sure it's working, let's modify our input:
```rs
let mut inputs = [input0, input1, input2, input3, input4, input5, input6];
for input in &mut inputs {
    input.wrapping_add(5);
}
```
Note that we use `wrapping_add` because it doesn't panic, unlike normal add which panics on overflow. We need to not let user mode programs cause a kernel panic no matter what they do.

Next, let's get the previous stack pointer that was store by the assembly code:
```rs
let user_mode_stack_pointer_ptr = get_local().user_mode_stack_pointer.get();
// Safety: the stack pointer was saved by the raw_syscall_handler
let user_mode_stack_pointer = unsafe { user_mode_stack_pointer_ptr.read() };
```
Finally, we set the output registers as well as some `sysretq` specific registers, set the stack pointer, and do `sysretq`:
```rs
unsafe {
    asm!(
        "
            mov rsp, {}
            sysretq
        ",
        in(reg) user_mode_stack_pointer,
        in("rcx") return_instruction_pointer,
        in("r11") rflags,
        in("rdi") inputs[0],
        in("rsi") inputs[1],
        in("rdx") inputs[2],
        in("r10") inputs[3],
        in("r8") inputs[4],
        in("r9") inputs[5],
        in("rax") inputs[6],
        options(noreturn)
    );
}
```

## Checking if it worked
Now let's run the code and use GDB on the user mode program again. After doing `target remote :1234`, run
```
info locals
```
This should output:
```
inputs_and_outputs = [15, 25, 35, 45, 55, 65, 75]
```
We successfully and securely did a syscall without messing up the ordering of the register inputs!

# Learn more
- https://nfil.dev/kernel/rust/coding/rust-kernel-to-userspace-and-back/. Warning: syscall handler implementation is unsound.
- https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI
- https://wiki.osdev.org/System_Calls
- https://en.wikipedia.org/wiki/System_call
- https://www.felixcloutier.com/x86/syscall
- https://www.felixcloutier.com/x86/sysret
