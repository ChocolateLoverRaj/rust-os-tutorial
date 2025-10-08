The previous part told us what registers and instructions are available to us to implement syscalls.  But we don't need to use all of them.

In this OS, we will have every `syscall` instruction be like a request, which is followed by a response on `sysretq`. We need a way to differentiate each type of syscall. We will use a *syscall number* for this. Each syscall needs an input. The output can be specific to the syscall. 

## Input
The input can basically be a pointer to memory, so that the input can be the same size (`usize`) regardless of the syscall number. We need to know the syscall number so that we can correctly interpret the input pointer. So we have this data structure:
```rs
struct SyscallInput {
    syscall_number: usize,
    pointer: usize,
} 
```
We need to pass this input when using the `syscall` instruction. The way that uses a minimal amount of registers would be to pass in a `*const SyscallInput` through a single register. Another way would be to use two registers, one for `syscall_number` and one for `pointer`.

When using the `syscall` and `sysretq` instructions, we can use up to 7 registers. But we don't have to use all 7. Depending on how many we use, both the user and kernel code can be optimized to only store and preserve certain registers. Certain syscalls might need more bytes to input data. If we pass data through registers, we might have some syscalls which use 1 register, and some that use 6 (in addition to 1 register for the syscall number). Right now our OS only runs on x86_64, which has up to 7 input and output registers for syscalls. Note that we could technically pass more `u64`s on the stack. 

If we optimize around the magic number `7`, it might cause issues later on if other architectures have less than 7 registers. We can assume that a register holds a `usize`, since on pretty much all architectures, the size of a register is `usize`. And currently the only value of `usize` are `u32` and `u64`. We are not trying to make this OS run on ancient 16-bit computers. So if we don't want to optimize for a specific architecture, then we will have x amount of `u32`s available to pass through registers.

Even though only 7 registers are available, we could always extend this number at the cost of performance by sending additional variables on the stack. So if there is an architecture in the future that only has 6 registers available, we can still use 1 `usize` on the stack. We can always use less registers than there are available.

At minimum, we need the actual input size to be `[usize; 2]`. One for the syscall number, and one for a pointer, which can be used in any way. It could be used as `Option<NonZero<usize>>`, as `usize`, or `u32`.

If we do always allocate additional registers as additional input, we *could* have increased performance if those additional registers are the difference between a process having to access user pointers or not access user pointers. But unless we can get some noticeable benefits from this, this just adds complexity to our assembly code.

So I decided that our syscall-ing convention will use 2 registers (on all architectures). The first register will be the syscall number. We need to keep the syscall number within the size of a `u32` so that we can easily support 32-bit architectures as well. The second register will be a `usize`, which, as mentioned before, can be used as a `u32` or a `usize` (pointer).

## Output
Some syscalls need an output. We need to send the output somehow. One way is to have the input pass in a pointer, and the output can just modify the memory. However, if the pointer itself is invalid (out of bounds or not mapped), then there may not be any way for the kernel to express the error.

We will figure this out later.

## Invalid syscall number
What happens if an invalid syscall is called? Do we return an error somehow?
