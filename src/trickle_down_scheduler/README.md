The scheduler created in this tutorial is going to be *experimental*. Instead of implementing a round robin scheduler, which most hobby operating systems do, we are going to make a unique one.

# Goals
- Instant responsiveness, even if the system is under high load (such as when you are running `cargo build` for the first time 😄).
- More important programs should not be slowed down by less important programs, no matter how CPU-intensive the less important programs are.
- There should not have to be a bunch of numbers to adjust for optimal performance.

# Name
I'm not sure if this kind of scheduler already has a name. I call it, "trickle down scheduler". It reminds me of how water flows. Ig it could also be called a "hierarchial scheduler".

# Design
## Task
A task is a runnable thing. A task is either a *container* or a *thread*.
```rs
enum Task {
    Container(Arc<Container>),
    Thread(Arc<Thread>)
}
```

## Thread
A thread is a thread as described in the previous part. The scheduler does not care which process a thread belongs to. One process could have a high priority thread and a low priority thread. A thread has a sub-task.
```rs
struct Thread {
    // Will contain more fields
    below: Option<Arc<Task>>,
}
```

## Container
A container can hold a single thread (and all sub-task), while also having a sub-task. A container is basically a chain of tasks with the ability to add a sub-task that runs if the entire chain is waiting, and will always be below the chain, even as tasks are added to the chain.
```rs
struct Container {
    inside: Arc<Task>,
    below: Option<Arc<Task>>
}
```

## Sub-task
When a thread does not need CPU (when it is waiting for something), then the sub-task is run. That sub-task could have another sub-task, and so there could be a chain of sub-tasks.

## Root task
There is only a single root task that the scheduler has to run. All tasks will be within this root task. While a thread can only use up to 1 CPU at a time, a task can use an unlimited number of CPUs, since sub-tasks can run in parallel to a task if there are multiple CPUs.

## Visual

This is how I would imagine a desktop system to run with this scheduler design. The root task is a service manager, which, similar to systemd, is in charge of starting other programs. The most important program is the window manager. The window manager itself manages 3 levels of priority. The most important is the window manager's own thread. This will make alt-tabbing and other window management very responsive. Next, the active (focused) window. Then, other visible windows (there could be split screen views, where there is another window that is visible but not focused). Finally, threads which could send notifications can run. Every app that has background notification threads could have their threads be run, even if the app's window is not in view.

## Running a thread
It is the scheduler's job to run the root task. You can run more threads by attaching them below the root task. So it is easy for a program to start, pause, and resume execution of a specific thread. All you need to do to pause a thread is to detach it. Then to resume it, you reattach it. Where in the "tree" you attach a thread determines its priority. All priority is relative. There are no numbers.

## Permissions
### Thread owner
The thread owns itself. Therefore, the thread's *process* owns the thread.

### Container owner
Whatever process created the container owns the container.

### Task owner
If the task is a thread, then the task owner is the thread owner. If the task is a container, then the task owner is the container owner.

### Attaching below
To attach a thread below a task, you need two permissions. You need permission to attach anything below a task. Only the owner of a task can attach a task below.

In addition, you need permission for a task to be attached. Thread A from process A cannot attach thread B from process B unless process B gives it permission to. Process B might not want thread B to run, and process A might mess up process B if it can cause thread B to be run unexpectedly.

## Converting a thread into a container
You can convert a thread into a container with the thread `inside`. This way, you can place other programs below your current program and attach your own threads directly below your current thread, while the other program still runs below all of your threads.

## Yielding
We will not have a general `sched_yield` syscall that loosely just says "give other threads a chance now". All yields will be yields with a purpose. Whenever a thread yields, it must say, "don't wake me up unless *this* happens". That event could be a timeout, hardware interrupt, or a message from another thread.

## Syscall Handlers
Should syscall handlers have interrupts enabled? If interrupts are disabled, implementing syscall handlers is easy. You can have a single syscall handler stack per CPU, since you will never have to exit a syscall handler while saving its stack. You don't need to worry about deadlocks. However, this could cause low responsiveness if a low-priority task, which would normally be preempt-able, becomes un-preempt-able for a significant amount of time during a syscall.

Enabling interrupts during a syscall handler could increase responsiveness, but it will be hard to implement. Making the syscall handler preempt-able would require having a kernel stack for every thread! This would use much more memory per thread. Also, the syscall handlers have to be very careful to not enable interrupts in a way that could cause deadlocks.

Some kernels manage the complexities of preempt-able syscall handlers. They have locks in a way so that no deadlocks can happen.

[seL4](https://sel4.systems/About/seL4-whitepaper.pdf) (which is literally a perfect micro-kernel), in "5.1 General real-time support", says that seL4 has interrupts disabled during syscall handlers. Its reasoning for this is that seL4 instead makes syscall handler execution time very short, and making the syscall handlers preempt-able would result in a negligible improvement in responsiveness. sel4 also says that making syscall handlers preempt-able is very complex and is not worth the tiny reduction in latency.

Using seL4 as an example, this scheduler will also disable interrupts during syscall handlers. Preferably, we should also reduce the amount of work that syscall handlers do to reduce the amount of time that interrupts are disabled. 

## Creating new threads

## Creating new processes
This is tricky, because the new process has a completely different address space. However, it can't do anything with an empty address space. It at least needs an executable to be loaded into its address space. So a process needs to be able to manage an address space for a new process until it is spawned. The process needs to be given a controlled way to manage the new address space without letting it directly modify page tables.

## Syscalls 
We will have every `syscall` instruction be like a request, which is followed by a response on `sysretq`. We need a way to differentiate each type of syscall. We will use a *syscall number* for this. Each syscall needs an input. The output can be specific to the syscall. 

### Input
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

### Output
Some syscalls need an output. We need to send the output somehow. One way is to have the input pass in a pointer, and the output can just modify the memory. However, if the pointer itself is invalid (out of bounds or not mapped), then there may not be any way for the kernel to express the error.

We will figure this out later.

### Invalid syscall number
What happens if an invalid syscall is called? Do we return an error somehow?

## Handling Errors
### Page faults

# Implementation
## Initially
Initially, it's easy for each CPU to claim a task to run, "filling up" the higher priority tasks first before running tasks below.

## Attaching a new thread
New threads are always attached by other threads. The CPU running the thread that attached the thread is the only CPU that knows about this attachment initially. That CPU needs to somehow find a CPU that is not running any thread, or a CPU that is running the least important thread. This calculation should not take a long time, because that CPU might be running an important thread that it needs to get back to.

One way we can do this is by scanning the entire tree of threads to determine the CPU running the least important thread. 

Once we have found the least-busy CPU, we will send an IPI to it. Then, when that CPU receives an IPI, it can also re-scan the tree of threads.

## Race conditions
### Tree changing during least-busy CPU calculation
- One of the more busy CPUs might just stop executing a thread, making it now the least busy
- One of the more busy CPUs might switch to executing a lower priority thread, making it now the least busy


### Multiple new threads at the same time
