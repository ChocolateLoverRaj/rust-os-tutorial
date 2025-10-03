# Scheduler Interfaces
What if we created a separate module or crate for the scheduler? How would the kernel interact with the scheduler? How would syscalls interact with the scheduler?

## Simple Round Robin Scheduler
### Spawn thread
Adds a thread to the queue.

### Remove thread
Removes a thread from the queue. However, we need to be careful since a different CPU could be executing the thread we want to remove. One way to do this is to just mark the thread as "terminating" and let the CPU running it get interrupted by its periodic timer. Another way is to send an IPI to the CPU that's running it to more instantly terminate the thread. Either way, our interface needs a way of configuring and handling timer interrupts, and maybe also IPI(s).

### Mark a thread as "waiting"
