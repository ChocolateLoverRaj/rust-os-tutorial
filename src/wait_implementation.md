# Wait implementation

## Level 0 - check every address every time
This is so inefficient, it would basically be equivalent to making the wait syscall be a alias for `sched_yield`.

## Level 1 - global flag for pending update ⭐
You have
```rs
static UPDATE_COUNT: AtomicUsize = AtomicUsize::new(0);
```
When a thread waits, you set its state to waiting with the value of `UPDATE_COUNT`. Then you check all of the addresses to see if any changed in between. If an address changed, you instantly change the thread state to not waiting.

Whenever any watched address gets changed, you increment `UPDATE_COUNT`. You should also send an IPI to all other CPUs, since other CPUs might be sleeping and the scheduler might benefit from waking up other CPUs. This could be optimized by keeping track of which CPUs are sleeping and only sending an IPI to those CPUs. 

When checking the waiting state of a thread, you compare its update count number to the `UPDATE_COUNT`. If it is equal, it means that nothing changed since the last check. If it is less, then you can just change the waiting state to not waiting, and the worst thing that will happen is a spurious wake-up. 

When going in a round-robin, if you complete a full cycle and all threads are waiting, then you sleep the CPU. At this point, only an interrupt will cause `UPDATE_COUNT` to increase so it is safe to sleep the CPU. 

In fact, with this method, the kernel doesn't even have to keep track of the addresses that are being watched, since it wakes up all threads whenever any watched address changes.

This is what this tutorial will use.

## Level 2 - more fine-grained updates
If we want to let updates wake up some threads but not others, we will need to:
- In the kernel, keep track of which addresses are being watched
- Keep track of addresses that are in memory shared between multiple processes
- Involves concurrent data structures, possibly a concurrent hash map
 