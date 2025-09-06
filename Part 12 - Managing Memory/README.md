# Managing Physical and Virtual Memory
I will assume that you already know about physical and virtual memory, and how paging works on x86_64 to map virtual memory to physical memory. You can read the following to learn these things:
- https://os.phil-opp.com/paging-introduction/
- https://os.phil-opp.com/paging-implementation/
- https://wiki.osdev.org/Paging

So, what is the state of virtual and physical memory so far? Limine has given us a list of physical memory regions, which lets us know which ones are available for us to use. We've already used a region of physical memory for our global allocator heap. Limine set up page tables, and we have not modified them yet.

## Setting up our own page tables
We're going to be mapping virtual memory to physical memory. Currently, we don't really know which parts in virtual memory are already mapped. It could cause issues if we try to map a page which is already mapped. So we'll create a new, blank L4 page table. That way, we know exactly what should and shouldn't be used. However, we need to re-create the mappings that Limine made.