# Handles and permissions
## Permissions
You are probably familiar with the concept of permissions with existing operating systems and security features. Android and iOS apps need to request for your permission to access your camera, send you notifications, etc. In Windows, apps need to ask for permission to modify installed programs, and access certain network features. In Linux, Flatpak and Snaps can restrict which files and other permissions apps are allowed to access.

Some kernels can understand permissions at a high level, such as keeping track of which user a process belongs to. Micro-kernels, such as seL4, provide the bare minimum set of tools to control fine-grained permissions for all processes, and leave a lot of flexibility to the processes in how they will group and control permissions. For example, in Linux, all users have a UID, which is a number that is used internally instead of usernames. The Linux kernel is aware of which  processes belong to what UID. In seL4, the micro-kernel does not store a UID for processes.

In this tutorial, we will be making a secure micro-kernel with fine-grained permissions. The kernel only has to care about which *system resources* a process has access to, such as a PS/2 device, PCI device, or region of memory.

## Handles
Handles are basically fixed size `[u8; N]` ids to a specific type of resource. For example, many operating systems use file descriptors as a handle to an open file. We could use handles to reference shared memory, threads, and permissions / access to certain things.
