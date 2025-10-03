# Async Designs
We are designing and writing an operating system. It's not just a kernel anymore. We will be writing the kernel and applications, and we will need to design a way for the kernel and application to communicate with each other.

A thread isn't always constantly executing code. Often, code is waiting for something to happen until it executes code. That thing that it is waiting for is either an interrupt, or some message from the kernel or another thread. In the previous section, we already talked about *yielding*. 

## `sched_yield`
We could create a syscall similar to Linux's `sched_yield` that just tells the kernel, "you can execute other threads before coming back to me". However, this gives no indication to the kernel about what the thread is waiting for or *when* the kernel should execute the thread again. When there is only one thread, the kernel will instantly go back to executing this process, which is very inefficient because it will result in very high CPU usage when waiting for anything. 

## `sleep`-based waiting
We could create a syscall that just blocks execution of the calling thread for a specified amount of time, and the kernel can execute other threads while a thread is sleeping. This way, when we loop until something happens in a thread, we can just wait a certain amount of time until checking again. This will reduce the amount of CPU usage when waiting for things, but will increase the maximum (and average) latency between an event happening and the thread handling it.

## Event-based waiting
All async events could be similar to a file descriptor in Linux. An event may be "ready" or "not ready". A thread can do a syscall to tell the kernel, "wake me up until one of these events happens". As a thread, this lets us be woken up exactly when we need to be woken up. Theoretically, we can achieve 0% CPU usage while waiting for events, and be able to wake up the thread as soon as we receive an interrupt (such as from a keyboard or a timer) or a message from another thread.

The kernel would have to keep track of events and assign an id to each event. When an interrupt happens, the kernel needs to mark the associated events as "ready". If any threads were waiting for an event that just happened, the kernel can then execute that thread again.

A potential race condition is if a thread checks if an event is ready and then calls the wait for event syscall, but the event happens in between checking for the event and calling the syscall. We can fix this in the kernel by making sure that a wait for event syscall instantly returns if one of the events is already "ready", and always waking up the thread whenever one of the events happens. The kernel can keep track of "pending" events, and mark those events as processed after a thread gets woken up from that event.

While this method of doing async does achieve minimum CPU usage and latency, keeping track of pending, ready, and not ready events in the kernel can be complicated. The kernel also has to keep track of which threads are waiting on what event. And because the "wait for event" syscall can modify the list of pending events, it can cause problems when multiple threads want to wait for the same event.

## `futex`-based waiting
`futex` is actually a Linux-specific term. It is meant to be used for "fast user-space locking". It works by sharing memory between two threads. A thread can tell the kernel, "wake me up when the `u32` value at this memory address changes to not be 0x123". The "fast" part of a futex is that in many cases, a syscall is not needed. Since the user-mode thread can directly access the memory, it can check its value without any syscalls.

Example: one thread is sending messages to another thread. It uses a shared `bool` (`pending_message`) to keep track of if a new message is available. The consuming thread will do an atomic swap from `true` to `false`. If the value was `true`, then that means there are message to process, and the consuming thread will process the messages, and then check again if there are new pending messages. If the value is `false`, the consuming thread will use the `futex` syscall to say to the kernel, "wake me up when the value at this address is not `false`". When the producing thread wants to change the value from `false` to `true`, it will also tell the kernel through a syscall to set that memory to `true`, and also check if the kernel should wake up the consuming thread.

We could use a similar method of checking if data is available and waiting for it to be available in our OS. Every source of data can be through memory, and we can wait until more data is available through a `futex`-like syscall.

In Linux 6.17, you can only wait for 1 futex at a time. There are plans to have add support for waiting for any of the memory regions in a list of memory regions to change. In our OS, we can add support for waiting for multiple locations.

A futex removes the confusion behind "pending events" because user-space manages how it will keep track of which events are pending. Different multi-thread data structures will have their own ways of synchronizing and signaling to other threads, all done through shared memory and futexes.
