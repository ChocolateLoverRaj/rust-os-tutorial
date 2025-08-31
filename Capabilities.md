# Capabilities
Similar to a file descriptor (fd), a capability is a handle to a resource, and also works as permission to *use* that resource. Another cool thing about capabilities is that you can share capabilities with other processes. It can be a nice abstraction for implementing resource sharing and limiting between processes.


## Cloning and sending / sharing capabilities
In Rust, you cannot send `T` to another thread and keep `T` too, unless `T` is `Copy` or you `Clone` `T`. Similarly, you cannot make another process be able to access a capability while also owning the capability yourself. That would be like if the capability implemented `Copy`. Capabilities have owners, and having multiple owning processes would be complicated. 

We can make some capabilities implement `Clone`. For example, the "Create PS/2 Event Stream" capability can be `Clone`. So a process can give this capability with another process while keeping it too by cloning its capability and sending it to the other process.

So what about `Send`? Can all capabilities be sent to a different thread? And does it make sense for a capability to implement `Clone` but not `Send`, or `Send` but not `Clone`?

## List of capabilities
### Create PS/2 Event Stream Capability
This capability basically says that the processor has permission to get input from the PS/2 keyboard and mouse. You can create an event stream, which is a ring buffer of shared mem between the process and the kernel. After the event stream is created, the event stream itself has a capability assigned to it.

Clone-able: true
Send-able: true

### PS/2 Event Stream Capability
This capability would definitely be used to "`Drop`" an event stream.

Clone-able: true. The kernel would have to also map the shared memory for another process. For this to be more consistent, we should probably not map the shared mem to the calling process when creating the event stream.
Send-able: ?. If the stream is already mapped, then it would not make sense to be able to send it without unmap-ing the mem. We should probably make this one *conditionally* sendable. Only sendable if it is not mapped. Then we can return an error if a process tries to send it while it's mapped. 

### Notifier capability
A notifier is a mechanism for one thread to send an event to another thread. We can make a capability for who can send notifications for a notifier. We can also make a capability for who can receive notifications on a notifier. Or we can have a single capability for both sending and receiving.

#### Notifier send
Clone-able: true
Send-able: true

#### Notifier receive
Clone-able: true
Send-able: true

## Event Id vs Capability Id
We definitely need an event id for any event we are going to wait for. Similar to how the Linux `read` and `epoll` syscalls are associated with the fd, our event ids are also associated with capabilities, such as an event stream capability or notifier capability.

One option is to not have a separate *event* id, and just make some capability ids work as events. For example, the "Create PS/2 Event Stream" capability would not work as an event id, but the "PS/2 Event Stream" and "Notifier Receiver" capabilities would work as event ids. I'm going to go with this option because it simplifies the user mode code.
