# We need something
Currently, our user mode program can't do anything useful. It can't log messages to the serial port. It can't draw to the screen. It cannot handle interrupts. This is good, because the kernel can restrict what a user mode program can and can't do. We need a way of allowing the user mode program do do certain things, such as log messages. We can have the user mode program *ask* the kernel so that the kernel can do stuff that only the kernel has permission to do, such as accessing the serial port. But how can the user mode program communicate with the kernel? With syscalls!

# What is a syscall?
A syscall is similar to calling a function, except that the user mode program calls the kernel's function. The CPU switches from user mode to kernel mode, while sending data from the user program. Then the kernel processes the program's message, and can "return" back to the program, switching from kernel mode to user mode, while sending data from the kernel.

How it works is the user mode program uses the `syscall` instruction, which switches execution to the kernel's syscall handler function. The `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`, and `rax` registers are free to be used when doing `syscall`, so a program can send data by writing to those registers before the `syscall` instruction. Then, the kernel can read those registers. The kernel can return back to the user program using the `sysretq` instruction. Again, the kernel can write to those registers before the instruction and the user program can read those registers after its `syscall` instruction.

In most operating systems, each `syscall` instruction is like a *request* from the user program, and the kernel responds to every request, sending a *response* back with a `sysretq`. In Linux, the `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`, and `rax`  registers are used for the *request*. `rax` specifies a syscall number, which specifies what type of request this is. `rdi`, `rsi`, `rdx`, `r10`, `r8`, and `r9` are used for up to 6 arguments, and each argument can be up to a `u64` in size. Then the kernel processes the request and sets `rax` to the *response*, which is up to a `u64` in size. However, operating systems can decide how they want to use `syscall` and `sysretq`. They can define their own use of the 7 registers, or even choose to not use registers to pass data!

# Tryout out the `syscall` and `sysretq` instructions
In the user mode program, add
```rs
fn syscall(inputs_and_ouputs: &mut [u64; 7]) {
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
This let's us write to the 7 registers before the `syscall` instruction, and see what the kernel set their values to before doing `sysretq`.

And then in `entry_point`, before our loop, let's try doing a syscall:
```rs
let mut inputs_and_outputs = [10, 20, 30, 40, 50, 60, 70];
syscall(&mut inputs_and_outputs);
syscall(&mut inputs_and_outputs);
```
This should let us view the inputs in the kernel, and then view the outputs in the kernel because they will be inputs the 2nd time.

# Processing the syscall
We have to have a syscall handler function, and then tell the CPU the address of the function. 

## Assembly function
We can't directly use a Rust function for the syscall handler. First, we need to switch the stack pointer to a known good stack that only our kernel can access. This way, the kernel will always run the syscall handler with a valid `rsp`, and the user program cannot inspect the kernel's stack after the syscall.

We'll have to write our syscall handler in assembly up to the point where we can safely call a Rust function.

We'll use a `naked` function to write the syscall handler in assembly while still integrating with Rust. Create `syscall_handler.rs`:
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
Before we do anything instruction involving the stack, such as `push`, `pop`, or `call`, we need to switch stacks. We can load the syscall handler's stack pointer from the CPU local data, and reference `GsBase` in our assembly code. But we also need to keep our current `rsp` value somewhere.  In `CpuLocalData`, add:
```rs
pub syscall_handler_stack_pointer: AtomicU64,
pub syscall_handler_scratch: AtomicU64,
```
And we can initialize both to `Default::default()`.

Next let's create an `init` function to initialize the syscall handler:
```rs
pub fn init() {
    let local = get_local();
    let syscall_handler_stack = GuardedStack::new(
        64 * 0x400,
        StackId {
            _type: StackType::SyscallHandler,
            cpu_id: local.kernel_assigned_id,
        },
    );
    local
        .syscall_handler_stack_pointer
        .store(syscall_handler_stack.top().as_u64(), Ordering::Relaxed);

    // Enable syscall in IA32_EFER
    // https://shell-storm.org/x86doc/SYSCALL.html
    // https://wiki.osdev.org/CPU_Registers_x86-64#IA32_EFER
    unsafe {
        Efer::update(|flags| {
            *flags = flags.union(EferFlags::SYSTEM_CALL_EXTENSIONS);
        })
    };

    // This tells the CPU the address of our syscall handler
    LStar::write(VirtAddr::from_ptr(raw_syscall_handler as *const ()));
}
```
This function also creates a guarded stack for the syscall handler.

Now let's update the assembly function:
```rs
#[unsafe(naked)]
unsafe extern "sysv64" fn raw_syscall_handler() -> ! {
    naked_asm!(
        "
            // Save the user mode stack pointer
            mov gs:[{syscall_handler_scratch_offset}], rsp
            // Switch to the kernel stack pointer
            mov rsp, gs:[{syscall_handler_stack_pointer_offset}]
        ",
        syscall_handler_scratch_offset = const offset_of!(CpuLocalData, syscall_handler_scratch),
        syscall_handler_stack_pointer_offset = const offset_of!(CpuLocalData, syscall_handler_stack_pointer),
    )
}
```
Here are writing to a memory location specified by the value of `GsBase` + an offset, and we use the [`offset_of!`](https://doc.rust-lang.org/nightly/core/mem/macro.offset_of.html) macro to get the offset.

We are almost ready to call a Rust function. We must pass some input to the Rust function. The Rust function needs:
- The 7 input registers (`rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`, `rax`)
- The pointer to the instruction that we should return to when returning from the syscall (currently stored in `rcx`)
- The value of the RFLAGS (currently stored in `r11`)
- The stack pointer to restore when returning from the syscall

| Calling convention                                                                   | `input[0]` | `input[1]` | `input[2]` | `input[3]` | `input[4]` | `input[5]` | `input[6]` | Additional inputs              |
|--------------------------------------------------------------------------------------|------------|------------|------------|------------|------------|------------|------------|--------------------------------|
| Our `syscall` (we decide the order and usage)                                        | `rdi`      | `rsi`      | `rdx`      | `r10`      | `r8`       | `r9`       | `rax`      | N/A                            |
| [`sysv64`](https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI) | `rdi`      | `rsi`      | `rdx`      | `rcx`      | `r8`       | `r9`       | N/A        | On the stack, in reverse order |

As you can see, we can directly pass `rdi`, `rsi`, `rdx`, `r8`, and `r9` to our Rust function without modifying those registers. For `input[4]`, we can set `rcx` to the value of `r10`. We can pass `rax` as an additional input on the stack. We can also pass `rcx` `r11`, and the previous stack pointer as additional inputs on the stack. This is how to do it in assembly, keeping in mind that additional arguments go on the stack **in reverse order**:
```
// This is input[9]
push gs:[{syscall_handler_scratch_offset}]
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
    return_stack_pointer: u64,
) -> ! {
    let mut inputs = [input0, input1, input2, input3, input4, input5, input6];
    log::debug!("Inputs: {inputs:?}");
    for input in &mut inputs {
        *input = input.wrapping_add(1);
    }
    todo!()
}
```
Note that we use `wrapping_add` because it doesn't panic, unlike normal add which panics on overflow. We need to not let user mode programs cause a kernel panic no matter what they do.

Now, from our assembly function, we can call our Rust function:
```
call {syscall_handler}
```
and we can do `syscall_handler = sym syscall_handler` to make our assembly function reference the pointer to our Rust function.

## Returning from the syscall
We just need to load the values in our 7 "output registers", and also restore `rsp` to the previous stack pointer, `rcx` to the previous instruction pointer, and `r11` to the previous rflags register value.
```rs
unsafe {
    asm!(
        "
            mov rsp, {}
            sysretq
        ",
        in(reg) return_stack_pointer,
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

# Checking if it worked
We should now see
```rs
[0] DEBUG Inputs: [10, 20, 30, 40, 50, 60, 70]
[0] DEBUG Inputs: [11, 21, 31, 41, 51, 61, 71]
```
The first time, we receive all of the expected numbers in the correct order, and the 2nd time, they are also in the correct order, and incremented by 1.

# Next steps
This part demonstrates that we can pass up to 7 registers as inputs and up to 7 registers as outputs. As part of the process of designing an OS, we have to choose how we are going to use those registers, and how many out of the 7 we are going to use. Are we going to have 1 register to indicate the "syscall number"? What if our input cannot fit in the 7 registers? These are things to consider.

# Learn more
- <https://nfil.dev/kernel/rust/coding/rust-kernel-to-userspace-and-back/>. Warning: syscall handler implementation is unsound.
- <https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI>
- <https://wiki.osdev.org/System_Calls>
- <https://en.wikipedia.org/wiki/System_call>
- <https://www.felixcloutier.com/x86/syscall>
- <https://www.felixcloutier.com/x86/sysret>
