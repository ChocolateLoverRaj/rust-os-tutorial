# What is a scheduler?
Currently, we have 1 program, if we can even call it that. We will need our OS to be able to run multiple programs at the same time. We should be able to start, pause, resume, and terminate programs. There are two important concepts relating to this (that most Rust developers probably already know).

A thread is the smallest unit of execution. Threads can run in parallel to other threads. Each thread has its own stack and registers.

A process is a group of threads that share the same address space, and therefore the same permissions. Any thread within a process can access all data accessible to other threads within the process. Processes cannot access the memory of other processes. This way, processes are isolated. Usually, a process is the same as "a program". However, some programs can run multiple processes, such as a video-related program having a UI as well as ffmpeg commands running in the background.

The *scheduler* is in charge of deciding which threads to run, which CPUs to run them on, and when to pause running a thread and run a different one. One of the things that a scheduler has to do is decide how to run threads when there are more threads than there are CPUs. More advanced schedulers will also consider CPU cache, and avoid sending threads across different CPUs often.

# Thread State
When the scheduler switches the thread that it is running, it can't just change the instruction pointer. It has to save all registers from the current thread and then load the saved registers when switching to a different thread. The state can be obtained at the "entry points" of the scheduler - the syscall handler and interrupt handlers. It can then be restored before a `sysretq` or `iret` instruction.

# Yielding
When a thread no longer has anything it needs to immediately do, it *yields*, letting the scheduler know that is is done for now, and the scheduler can run other threads while the thread that yielded is waiting. One example is if a thread calls `sleep` (waits for a certain amount of time to pass before doing the next step). In this case, the thread yields so that while it is waiting on `sleep`, other threads can run. 

Another example is if a thread is waiting on keyboard input, which is especially common for terminal apps. The thread will yield, and it will not be executed until keyboard input is ready for the thread to process.

If all threads are waiting and no thread needs to be executed, the scheduler can tell the CPU to stop and enter a low-power state, such as with the `hlt` instruction (which we already used).

# Collaborative multitasking
Cooperative multitasking is a technique used to schedule threads. Threads have to *collaborate* with each other, and **must** yield in order for other threads to run. Threads need to be mindful that there are other threads that want to be scheduled, and should not hog the CPU. An advantage of cooperative multitasking is that it's easy to implement the scheduler. The scheduler does not have to forcefully take control of the CPU from the thread. It can just wait for the thread to yield. A huge disadvantage is that if a thread does not yield fast enough. The OS will be very unresponsive. Imagine alt-tabbing, but it takes 0.5s every time you alt-tab because the running thread took 0.5s to yield. If the thread never yields, which could be due to a malicious program or a bug, the OS is basically frozen. In modern times, it makes no sense to rely on collaborative multitasking, and the CPUs that our OS is targeting are not from the cooperative-multitasking time period.

# Preempting
Preemption is when the scheduler forcefully stops the running thread from running. With preemption, a thread can be stopped even if it doesn't yield. Preemption solves the problems of collaborative multitasking. If a program seems to be frozen, you can force close it. Even if a program is using the CPU, alt-tabbing can trigger scheduler to preempt the process and switch to a different process, making the OS feel much more responsive, no matter how much the current program hogs the CPU. Schedulers utilize interrupts to preempt threads. When the CPU receives an interrupt, control of the CPU is transferred from the running thread to the scheduler. The scheduler can then decide to not resume execution of the interrupted thread, and give control of the CPU to a different thread instead.

# Round robin scheduler
A round-robin scheduler has a constant periodic timer interrupt that it uses to preempt threads. A thread is allowed to run up to a specified maximum amount of time. If the thread does not yield within that time, it gets preempted, and gets sent to the end of the line. Then the next thread in line is executed. See the [Wikipedia article](https://en.wikipedia.org/wiki/Round-robin_scheduling) for a more detailed explanation. Most hobby operating systems implement a round robin scheduler. An advantage of a round-robin scheduler is that no thread can hog the CPU, and CPU-time is shared pretty fairly between threads. The OS is generally responsive, especially if there are not a large number of threads ready to be run. Round robin schedulers don't have to give every thread an equal amount of CPU time. They can give some higher priority threads more CPU time and lower priority threads less CPU time.

# Learn More
- [Introduction to RTOS Part 3 - Task Scheduling | Digi-Key Electronics](https://www.youtube.com/watch?v=95yUbClyf3E&list=PLEBQazB0HUyQ4hAPU1cJED6t3DU0h34bz&index=3)
- <https://en.wikipedia.org/wiki/Yield_(multithreading)>
- <https://en.wikipedia.org/wiki/Cooperative_multitasking>
- <https://en.wikipedia.org/wiki/Preemption_(computing)>
- <https://en.wikipedia.org/wiki/Interrupt>
- <https://wiki.osdev.org/Interrupts>
- <https://en.wikipedia.org/wiki/Round-robin_scheduling>
- <https://en.wikipedia.org/wiki/Scheduling_(computing)>
