This OS is going to not have any "blocking" methods such as sleeping for x time, or reading a file blocking, or joining a thread. It's going to be super `async` friendly. So we need to design a way that will make it easy to have an async  executor in user mode, also keeping performance in mind.

# Options
## **Events slice and return slice** - I chose to use this option
The wait until event syscall will input a `&mut [EventId]` for the events it will wait for, and also a `&mut [MaybeUninit<EventId>]` that the kernel will fill with the events that happened before returning from the syscall. During the execution of the syscall, the calling thread "owns" the events and other threads cannot also wait until the same events happen at the same time. 

### Advantages
- Simple to implement in user mode

### Disadvantages
- The kernel cannot read and write to the slices while it is running threads in different processes.
- User mode will likely have to make an allocation to fill the input events, even if the list is the same every time.

## `epoll`-like syscalls
- 1 syscall to create a list of events to watch (this list is managed by the kernel)
- 1 syscall to add an event to a list 
- 1 syscall to remove an event from the list
- 1 syscall to "read" events that happened from the list and block until at least 1 event happened
- Whenever an event is added to the list, the thread now owns the event.

### Advantages
- When the same events are being listened for over and over, the executor doesn't have to modify the list
- The kernel can manage the list state of events that happened, and access it even when it's address space is for a different process

### Disadvantages
- Many more syscalls when waiting for a lot of events.
- Involves kernel memory allocations
- Involves the kernel copying every event that happened to the user list

# Ideal Option
## Features
### Shared memory between kernel and user
The memory should be mapped to the kernel's address space too so that an interrupt / event source from any CPU can very quickly show that the event happened.

### Avoid copying / allocations every time wait until event is called
For high performance, high load scenarios, such as accessing a USB device, NVMe, or network card, the thread is going to be calling wait until event a lot (hopefully it can keep up with the events).

## Challenges
### Dynamically growing the size of shared memory.
Let's say that each entry for an event takes up 128 bytes. Then, if a 16 KiB shared memory mapping was used, then up to 128 events could be added. This will already be enough for most cases. However, we don't want to have an error if we for some reason need >128 events. We need to have a way of growing this. If we want to grow it without moving the virtual addresses, this requires reserving enough contiguous virtual memory to be able to grow it to the max size, and then allocating and mapping addition physical shared memory on demand.

### Stable data structure
Since the kernel and user mode program could be compiled using different versions of Rust, we can't just rely on Rust data structures *just working*. We will need a `#[repr(C)]` or something.

### Accessing events like a map
We need some sort of shared map structure to efficiently query events.
