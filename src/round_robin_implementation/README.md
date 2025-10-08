This will be how we implement our simple, multi-CPU, round-robin scheduler with futex-based waiting.

## List of tasks
Because there will be multiple CPUs accessing this list at the same time, different kinds of lists will have different trade-offs. We could use a `Mutex` or `RwLock`. This could have performance problems if a CPU is adding or removing a thread from the list while other CPUs want to read it. We *could* improve performance by using more complex lists that are more lock-free. Currently, our global allocator is also behind a spinlock, so that could also be a significant performance penalty. We will start simple and just use a `spin::RwLock`. 

## Global counter for updates
```rs
static UPDATE_COUNT: AtomicUsize = AtomicUsize::new(0);
```

## Global counter for threads that were preempted
```rs
static PREEMPT_COUNT: AtomicUsize = AtomicUsize::new(0);
```

## Timer
For simplicity, you can just use the local APIC timer as the time slice, or the constant tick that switches which thread to run. Turn on the APIC timer.

## Spawn thread
Adds a thread to the queue.

## Remove thread
Removes a thread from the queue. However, we need to be careful since a different CPU could be executing the thread we want to remove. One way to do this is to just mark the thread as "terminating" and let the CPU running it get interrupted by its periodic timer. Another way is to send an IPI to the CPU that's running it to more instantly terminate the thread. Either way, our interface needs a way of configuring and handling timer interrupts, and maybe also IPI(s). We'll start simple and not send an IPI.

## When a thread calls wait
When a thread waits, you set its state to waiting with the value of `UPDATE_COUNT`. Then you check all of the addresses to see if any changed in between. If an address changed, you instantly change the thread state to not waiting. You can then move on to executing the next thread.

## Executing threads
You try to lock the thread. If you acquire the lock, you check if the thread is ready or not ready. If the thread is not ready, you keep track of the thread id and the preempt count. Then you move on to the next thread and check if that one is ready. If you go through the entire list of threads and come back to the previous thread and the preempt count is the same, you disable the APIC timer interrupt and sleep the CPU.

When checking the waiting state of a thread, you compare its update count number to the `UPDATE_COUNT`. If it is equal, it means that nothing changed since the last check. If it is less, then you can just change the waiting state to not waiting, and the worst thing that will happen is a spurious wake-up. 

If the thread is ready, you set your CPU state to be running that thread, and then you enter user mode.

## Notify syscall
A thread will call this syscall whenever it changes an address that is being watched by another thread.

Whenever any watched address gets changed, you increment `UPDATE_COUNT`. You should also send an IPI to all other CPUs, since other CPUs might be sleeping and the scheduler might benefit from waking up other CPUs. This could be optimized by keeping track of which CPUs are sleeping and only sending an IPI to those CPUs. 
