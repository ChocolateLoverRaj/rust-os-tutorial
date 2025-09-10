# Guard Page
As mentioned in the part about page fault handling, our kernel currently has no protections against stack overflows.

Let's purposely create a stack overflow:
```rs
fn stack_overflow() {
    stack_overflow();
}
stack_overflow();
```
Since we have no guard page, running this code could result in *anything*. When I ran this code, I got an "Invalid Opcode" fault. But we know that the underlying cause was not an invalid opcode.

A guard page is just a page that is purposely left unmapped. This way, if a stack overflow happens, it will result in a page fault rather than silently overwriting other data.

## Allocating and mapping memory for a stack with a guarded page
Create `guarded_stack.rs`. In it, we will define a stack size:
```rs
pub const NORMAL_STACK_SIZE: u64 = 64 * 0x400;
```
This is 64 KiB, which is also Limine's default stack size.

Next, we create a struct that holds a guarded stack, which is a stack with a guard page.
```rs
/// A stack with a guard page at the bottom.
/// Dropping this does not unmap the stack.
#[derive(Debug)]
pub struct GuardedStack {
    top: VirtAddr,
}
```

Also create a file called `x86_64_consts.rs`. In it, we will define some useful numbers. For now:
```rs
pub const HIGHER_HALF_START: u64 = 0xFFFF800000000000;
```
We need to only use memory >= `HIGHER_HALF_START` for the kernel.

We need a way of allocating virtual memory. We can add a method in the `VirtualMemory` struct to do that:
```rs
impl VirtualMemory {
    /// Returns the start page of the allocated range of pages.
    /// Pages are guaranteed not to be mapped.
    pub fn allocate_contiguous_pages(
        &mut self,
        page_size: PageSize,
        n_pages: NonZero<u64>,
    ) -> Option<Page> {
        let interval = self
            .set
            .gaps_trimmed(iu(HIGHER_HALF_START))
            .find_map(|gap| {
                let aligned_start = gap.start().next_multiple_of(page_size.byte_len_u64());
                let interval = ii(
                    aligned_start,
                    aligned_start + (n_pages.get() * page_size.byte_len_u64() - 1),
                );
                if gap.contains_interval(&interval) {
                    Some(interval)
                } else {
                    None
                }
            })?;
        self.set
            .insert_merge_touching(interval)
            .expect("no overlap");
        Some(Page::new(VirtAddr::new(interval.start()), page_size).expect("should be aligned"))
    }
}
```
This function finds (aligned) contiguous pages of unused memory, mark them as used, and then return the starting page.

Next, let's add a function to create a `GuardedStack`:
```rs
impl GuardedStack {
    /// Locks physical and virtual memory to allocate the stack
    pub fn new(size: u64, id: StackId) -> Self {
        let memory = MEMORY.get().unwrap();
        let mut physical_memory = memory.physical_memory.lock();
        let mut virtual_memory = memory.virtual_memory.lock();
        let n_mapped_pages = size.div_ceil(STACK_PAGE_SIZE.byte_len_u64());
        let n_virtual_pages = n_mapped_pages + 1;
        let allocated_pages = virtual_memory
            .allocate_contiguous_pages(STACK_PAGE_SIZE, NonZero::new(n_virtual_pages).unwrap())
            .unwrap();
        todo!()
    }
}
```

Let's also keep track of the guard pages, so that we can detect which stack overflowed if a stack overflow happens:
```rs
#[derive(Debug, Clone, Copy)]
pub enum StackType {
    Normal,
    ExceptionHandler,
}

#[derive(Debug, Clone, Copy)]
pub struct StackId {
    pub _type: StackType,
    #[allow(unused)]
    pub cpu_id: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct StackInfo {
    #[allow(unused)]
    id: StackId,
    #[allow(unused)]
    size: u64,
}

pub const STACK_PAGE_SIZE: PageSize = PageSize::_4KiB;
pub static STACK_GUARD_PAGES: spin::Mutex<BTreeMap<Page, StackInfo>> =
    spin::Mutex::new(BTreeMap::new());
```

Back in our `new` method, let's insert the guard page:
```rs
// We purposely don't map the bottom page
// so that it causes a page fault instead of silently overwriting data used for other purposes
let guard_page = Page::new(allocated_pages.start_addr(), STACK_PAGE_SIZE).unwrap();
STACK_GUARD_PAGES
    .lock()
    .insert(guard_page, StackInfo { id, size });
```

Before we can create mappings, we need a way of getting a `&mut ez_paging::ManagedL4PageTable`:
```rs
impl VirtualMemory {
    pub fn l4_mut(&mut self) -> &mut ManagedL4PageTable {
        &mut self.l4
    }
}
```

Now in `GuardedStack::new`, we can map the stack:
```rs
let start_page = guard_page.offset(1).unwrap();
for i in 0..n_mapped_pages {
    let page = start_page.offset(i).unwrap();
    let frame = physical_memory
        .allocate_frame_with_type(
            STACK_PAGE_SIZE,
            MemoryType::UsedByKernel(KernelMemoryUsageType::Stack),
        )
        .unwrap();
    let flags = ConfigurableFlags {
        writable: true,
        executable: false,
        pat_memory_type: PatMemoryType::WriteBack,
    };
    let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
    unsafe {
        virtual_memory
            .l4_mut()
            .map_page(page, frame, flags, &mut frame_allocator)
    }
    .unwrap();
}
```
Create the `KernelMemoryUsageType::Stack` enum variant, so we can identify that physical memory as being used for a stack. Finally, we construct the `GuardedPage`:
```rs
Self {
    top: (start_page.start_addr() + n_mapped_pages * STACK_PAGE_SIZE.byte_len_u64()),
}
```

## Switching stacks
We wrote the code for creating a stack, but how do we use it? We need to change the value of the `rsp` register. The `sp` in `rsp` stands for "stack pointer". The `rsp` points to the top of the stack, and the stack goes down. We need to change `rsp` to point to the new stack. However, this can get messy, since once we switch to the new stack, we cannot reference any data that is stored on our old stack, and we need to make sure that the code that Rust generates doesn't implicitly reference it either. The best way is to call a new function that never returns. This way, we can be sure that the contents of the old stack are never accessed. To do this, let's create `call_with_rsp.rs`:
```rs
use core::arch::naked_asm;

/// # Safety
/// Stack must be valid
#[unsafe(naked)]
pub unsafe extern "sysv64" fn call_with_rsp(new_rsp: u64, f: extern "sysv64" fn() -> !) -> ! {
    naked_asm!(
        "
        mov rsp, rdi
        call rsi
        "
    );
}
```
This function, `call_with_rsp`, is a [naked function](https://blog.rust-lang.org/2025/07/03/stabilizing-naked-functions/), which is basically a function written entirely in assembly. We use assembly, and not Rust, to make sure that the Rust code does not implicitly end up messing with the stack. This tutorial will talk more about assembly and calling conventions later. For now, just know that the first input (in this case, `new_rsp`) gets passed through the `rdi` register. When we do `mov rsp, rdi`, it's basically like doing `rsp = rdi`. We are setting the value of `rsp` to the value provided through `new_rsp`. Next, we call the function `f`. `f` is the 2nd input, which is passed through the `rsi` register. So `call rsi` in assembly is like `f()` in Rust. We specify `extern "sysv64"` for `f` so that we can call it with the `call` instruction. In the end, calling `call_with_rsp` will result in the execution of function `f` on the new stack.

Let's use our new function to write the following method:
```rs
impl GuardedStack {
        pub fn switch(self, f: extern "sysv64" fn() -> !) -> ! {
        let new_rsp = self.top.as_u64();
        // Safety: The worst that can happen is a stack overflow, since we mapped a guard page
        unsafe { call_with_rsp(new_rsp, f) }
    }
}
```

## Switching stacks on the BSP
In `main.rs`, let's move some code to a new function:
```rs
extern "sysv64" fn init_bsp() -> ! {
    gdt::init();
    idt::init();

    let mp_response = MP_REQUEST.get_response().unwrap();
    for cpu in mp_response.cpus() {
        cpu.goto_address.write(entry_point_ap);
    }

    hlt_loop();
}
```
This function will run on a guarded stack, so if this function causes a stack overflow, we can detect it with a page fault handler.

Then we just need to call the function on a guarded stack inside `entry_point_bsp`:
```rs
GuardedStack::new(
    NORMAL_STACK_SIZE,
    StackId {
        _type: StackType::Normal,
        cpu_id: get_local().kernel_assigned_id,
    },
)
.switch(init_bsp)
```

Let's do the same for all APs:
```rs
extern "sysv64" fn init_ap() -> ! {
    gdt::init();
    idt::init();

    hlt_loop()
}
```
and in the bottom of `entry_point_ap`:
```rs
GuardedStack::new(
    NORMAL_STACK_SIZE,
    StackId {
        _type: StackType::Normal,
        cpu_id: get_local().kernel_assigned_id,
    },
)
.switch(init_ap)
```

We are close to nicely debugging stack overflows, but not yet. At this point, if we call `stack_overflow();` before `hlt_loop()` in `init_bsp`, it will still result in a triple fault.

## Interrupt Stack Table (IST)
By default, when an interrupt or exception occurs, the CPU will continue using the current stack, and execute the interrupt handler from the current value of `rsp`. Normally, when the stack is valid and has enough stack space for the interrupt handler to run, this is completely fine. The memory below `rsp` is free to be used by the interrupt handler. When the interrupt handler returns, the `rsp` is back to where it was before the interrupt, and the interrupted function can continue using the stack without problems. However, when a stack overflow happens, the `rsp` is already outside of the allocated stack. In our case, the page fault handler will try to push data to the stack, causing another page fault. This nested page fault will lead to a double fault, and then a triple fault.

The IDT provides a mechanism to make sure that the page fault handler is run on a valid stack. You can tell the CPU, "when a page fault happens, set `rsp` to this value when calling my page fault handler". The way this is done is kind of complicated. Remember the TSS, or `TaskStateSegment`, referenced in the GDT? The TSS contains an IST (Interrupt Stack Table). This "table" contains up to 7 slots where you can store the `rsp` of a stack. Then, in the IDT, for each entry, you can define a slot index in the IST for the stack that you want to switch to for the interrupt handler.

```
GDT
└── Pointer to TSS

TSS
└── IST
    ├── Slot 0 (pointer to the top of a stack)
    ├── Slot 1 (pointer to the top of a stack)
    ├── Slot 2 (pointer to the top of a stack)
    ├── Slot 3 (pointer to the top of a stack)
    ├── Slot 4 (pointer to the top of a stack)
    ├── Slot 5 (pointer to the top of a stack)
    └── Slot 6 (pointer to the top of a stack)

IDT
├── IDT Entry 0
│   ├── Stack Index
├── IDT Entry ...
```
The stack index in an IDT entry can have a value of `0..=7`. If it's `0`, it means "don't switch `rsp`". If it is `n` where `n` > `0`, it means "set `rsp` to the value defined in slot `n-1` of the IST.

Let's start with defining stacks in the IST. We have 7 "slots", so let's define an enum for how we will use those slots in `gdt.rs`:
```rs
#[derive(Debug, IntoPrimitive)]
#[repr(u8)]
pub enum IstStackIndexes {
    Exception,
}
```
To convert the enum value to a `u8`, we will use the `num_enum` crate:
```toml
num_enum = { version = "0.7.4", default-features = false }
```

Next, back in `guarded_stack.rs`, let's define
```rs
pub const EXCEPTION_HANDLER_STACK_SIZE: u64 = 64 * 0x400;
```
Again, 64 KiB. We might not need this much, but 64 KiB isn't much memory anyways.

Let's also create a `StackType::ExceptionHandler` enum variant.

Back in `gdt.rs`, we can add stuff to the TSS:
```rs
let tss = local.tss.call_once(|| {
    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[u8::from(IstStackIndexes::Exception) as usize] =
        GuardedStack::new(
            EXCEPTION_HANDLER_STACK_SIZE,
            StackId {
                _type: StackType::ExceptionHandler,
                cpu_id: local.kernel_assigned_id,
            },
        )
        .top();
    tss
});
```
When we create a dedicated stack for the page fault handler, we can use this opportunity to again create a `GuardedStack`, so that even if the page fault handler itself causes a stack overflow, it won't silently overwrite data. We just need to add a method to get a pointer to the top of the stack:
```rs
impl GuardedStack {
    pub fn top(&self) -> VirtAddr {
        self.top
    }
}
```

Now in `idt.rs`, we can adjust how we define our page fault handler to specify the stack index:
```rs
unsafe {
    idt.page_fault
        .set_handler_fn(page_fault_handler)
        .set_stack_index(u8::from(IstStackIndexes::Exception).into())
};
```
Note that the `set_stack_index` accepts the actual index into the IST, and adds `1` internally.

Now, finally, we can call our `stack_overflow` call this function in `init_bsp`, before `hlt_loop()`. This should result in a panic message saying a page fault happened. No strange other exceptions. No double faults. No triple faults. Nice.

## Logging stack overflows
Logging a stack overflow with the accessed address is useful, but it would be even more useful if the page fault handler automatically identified a stack overflow for us. Let's convert our `idt.rs` into `idt/mod.rs` and move the page fault handler to `idt/page_fault_handler.rs`.
```rs
pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let accessed_address = Cr2::read().unwrap();
    if let Some(stack) = STACK_GUARD_PAGES
        .lock()
        .iter()
        .find_map(|(page, stack_id)| {
            if accessed_address.align_down(page.size().byte_len_u64()) == page.start_addr() {
                Some(*stack_id)
            } else {
                None
            }
        })
    {
        panic!("Stack overflow: {stack:#X?}");
    } else {
        panic!(
            "Page fault! Stack frame: {stack_frame:#?}. Error code: {error_code:#?}. Accessed address: {accessed_address:?}."
        );
    }
}
```
We use the stored `STACK_GUARD_PAGES` to check if the accessed address which caused the page fault actually accessed the guard page. All page faults will have an accessed access within a guard page. Technically, it is possible for a non-page fault to access a guard page, resulting in the page fault handler to report that a page fault handler happened when it actually didn't happen. However, this is very unlikely, so it would be more useful to just assume that a stack overflow happened.

## Trying it out
Try causing a stack overflow in one of the functions that runs on a guarded stack. You should see a message like this:
```
[0] ERROR panicked at kernel/src/idt/page_fault_handler.rs:24:9:
Stack overflow: StackInfo {
    id: StackId {
        _type: Normal,
        cpu_id: 0x0,
    },
    size: 0x10000,
}
```
