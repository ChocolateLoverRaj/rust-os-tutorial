# Key Events
## `wait` syscall
This syscall lets a thread wait until one of any of a specified list of conditions (`futex`) is met. The kernel is allowed to do spurious wake-ups. The kernel does not return anything indicating which condition was met.

## `notify` syscall
This syscall notifies the kernel that the thread changed the value at a specific memory address, and that the kernel should check if any other threads that were waiting on this value should be executed again.

## External interrupts
Interrupts external to the OS (basically all interrupts excluding IPIs), such as a PS/2 interrupt or PCI interrupt. The kernel provides an interface for user mode to be access external devices, and includes a mechanism to notify / wake up a thread when external data is available, or when an external interrupt happens. The kernel will do this through accessing user memory in a `futex`-friendly way. In this case, the kernel may internally call `notify`, similar to the `notify` syscall.

# Implementation
## Choosing a thread to run
Initially, it's easy for each CPU to claim a task to run, "filling up" the higher priority tasks first before running tasks below.

## Switching threads
There are two reasons why a CPU would want to switch threads:
- The current thread should no longer be run
    - It transitions from ready to waiting
    - It exits
    - It crashes
    - It gets detached from the tree
- A higher priority thread should now be run
    - It transitions from waiting to ready
    - It gets attached to the tree
    - A different CPU that was running the thread is no longer running the thread (depending on the implementation, this should or should't happen)

## Switch causes originating on the running CPU
- The running thread transitions from ready to waiting
- The running thread exits
- The running thread crashes

All of these causes don't require an IPI.

## Switch causes that may originate on a different CPU
- The running thread gets detached from the tree 
- A higher priority thread transitions from waiting to ready
- A higher priority thread gets attached to the tree
- A higher priority thread gets dropped by a different CPU

If the cause originates on a different CPU, then an IPI will need to be sent to the CPU that is running the thread. The challenge is to efficiently send an IPI to the needed CPUs, without unnecessarily interrupting CPUs that don't need to be interrupted.

## Simple and inefficient method - IPI all!
> The challenge is to efficiently send an IPI to the needed CPUs, without unnecessarily interrupting CPUs that don't need to be interrupted.

We can just not do the challenge. To keep things simple, we could just send an IPI to all other CPUs whenever another CPU *might* need to switch threads.

### When this works great
- When we are only running 1 thread
- When we only have 1 CPU (will always be a no-op)
- When we are not frequently modifying the task tree
- When the highest priority tasks are the ones that wait often, and are very unlikely to be preempted because they barely consume any CPU time

### When this will have issues
- When lower priority tasks notify very often, this will preempt high-priority tasks often
- If multiple high-priority threads are constantly getting preempted, they might switch which CPU they get run on often. Both CPUs will have to constantly switch address spaces, resulting in performance loss.
- When the task tree is modified often, causing every CPU to recompute which thread it should be running very often, even when it shouldn't have to switch which thread it should be running. 
