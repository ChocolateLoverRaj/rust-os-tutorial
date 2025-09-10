# What is user mode?
So far, our operating system has been just a kernel. In an OS, the kernel is code that runs with full permissions. The kernel can modify page tables, access any memory, and access any I/O port.

Part of what an operating system is expected to do is run arbitrary, untrusted programs without letting those programs do bad things such as cause a kernel panic, triple fault, and access things it's not allowed to. To do this, CPUs have a thing called a privilege level. In x86_64, there are two privilege levels used in modern day operating systems. Ring 0, which is kernel mode, and Ring 3, which is user mode. 

User mode is capable of running programs with restricted permissions, so that code running in user mode cannot mess up your computer, kernel, or other programs (as long as the kernel doesn't have security vulnerabilities that let user mode code bypass restrictions). User mode is essential for running code that you don't fully trust. User mode also helps contain the damage caused by buggy code, making it so that at worst, a program will just crash itself and not crash the kernel or other programs. Even if you're planning on only running your own code that you trust on your OS, you should still run as much of your code as you practically can in user mode.

# Programs and executable file formats
You should be familiar with programs. Whether it's a graphical app, a command line tool, or a systemd service, all apps are made up of at least 1 executable file. Before we can start *executing* a program in user mode, we need to load it into memory. At minimum, we need to load the executable parts of a program (which contains CPU instructions), immutable global variables (`const`), mutable global variables (`static`), and memory for the stack.

There are many different file formats that store information about these memory regions. For our OS, we will be using the ELF format for our programs. This format is widely used in operating systems, including Linux, FreeBSD, and [RedoxOS](https://www.redox-os.org/). It is also the format that Rust outputs when building code for the `x86_64-unknown-none` target.

# Creating a program
In your workspace's `Cargo.toml` file, add a workspace member `"user_mode_program_0"`. Then create a folder `user_mode_program_0` with `Cargo.toml`:
```toml
[package]
name = "user_mode_program_0"
version = "0.1.0"
edition = "2024"
publish = false

[[bin]]
name = "user_mode_program_0"
test = false
bench = false
```
and `src/main.rs`:
```rs
#![no_std]
#![no_main]

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[unsafe(no_mangle)]
unsafe extern "sysv64" fn entry_point() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
```
Similar to our original kernel program, we'll start with a very minimal Rust program (which needs a panic handler to compile). To indicate that the `entry_point` function is the entry point in our program, create `build.rs`:
```rs
fn main() {
    // Tell cargo to specify in the output ELF what the entry function is
    let entry_function = "entry_point";
    println!("cargo:rustc-link-arg=-e{entry_function}");
}
```

Let's build it:
```bash
cargo build --package user_mode_program_0 --target x86_64-unknown-none
```
Now we can look at the generated code:
```bash
objdump -d target/x86_64-unknown-none/debug/user_mode_program_0
```
```
target/x86_64-unknown-none/debug/user_mode_program_0:     file format elf64-x86-64


Disassembly of section .text:

0000000000201120 <entry_point>:
  201120:       eb 00                   jmp    201122 <entry_point+0x2>
  201122:       f3 90                   pause
  201124:       eb fc                   jmp    201122 <entry_point+0x2>
```
We can see that our `entry_point` got compiled into 3 instructions, which make the forever loop.

Now let's look at the ELF file info needed by our kernel to load and run the program:
```bash
readelf --file-header --segments target/x86_64-unknown-none/debug/user_mode_program_0
```
```
ELF Header:
  Magic:   7f 45 4c 46 02 01 01 00 00 00 00 00 00 00 00 00 
  Class:                             ELF64
  Data:                              2's complement, little endian
  Version:                           1 (current)
  OS/ABI:                            UNIX - System V
  ABI Version:                       0
  Type:                              EXEC (Executable file)
  Machine:                           Advanced Micro Devices X86-64
  Version:                           0x1
  Entry point address:               0x201120
  Start of program headers:          64 (bytes into file)
  Start of section headers:          4504 (bytes into file)
  Flags:                             0x0
  Size of this header:               64 (bytes)
  Size of program headers:           56 (bytes)
  Number of program headers:         4
  Size of section headers:           64 (bytes)
  Number of section headers:         13
  Section header string table index: 11

Program Headers:
  Type           Offset             VirtAddr           PhysAddr
                 FileSiz            MemSiz              Flags  Align
  PHDR           0x0000000000000040 0x0000000000200040 0x0000000000200040
                 0x00000000000000e0 0x00000000000000e0  R      0x8
  LOAD           0x0000000000000000 0x0000000000200000 0x0000000000200000
                 0x0000000000000120 0x0000000000000120  R      0x1000
  LOAD           0x0000000000000120 0x0000000000201120 0x0000000000201120
                 0x0000000000000006 0x0000000000000006  R E    0x1000
  GNU_STACK      0x0000000000000000 0x0000000000000000 0x0000000000000000
                 0x0000000000000000 0x0000000000000000  RW     0x0

 Section to Segment mapping:
  Segment Sections...
   00     
   01     
   02     .text 
   03     
```
In our kernel, we'll need
```
Entry point address:               0x201120
```
because that's the address to the instruction that we will tell the CPU to start executing code in user mode.

Now let's look at the program headers, aka segments. Our kernel will only need to process the segments that are type `LOAD`. Looking at the flags, we can see that our ELF has two segments. One that is read-only and one that is read and execute. Once we use `static` global variables in our program, there will be another `LOAD` segment with read-write flags. Let's force there to be a `RW` `LOAD` segment:
```rs
#![no_std]
#![no_main]

use core::{hint::black_box, sync::atomic::AtomicU8};

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

static TEST_VAR: AtomicU8 = AtomicU8::new(0);

#[unsafe(no_mangle)]
unsafe extern "sysv64" fn entry_point() -> ! {
    black_box(&TEST_VAR);
    loop {
        core::hint::spin_loop();
    }
}
```
Now our segments look like this:
```rs
Program Headers:
  Type           Offset             VirtAddr           PhysAddr
                 FileSiz            MemSiz              Flags  Align
  PHDR           0x0000000000000040 0x0000000000200040 0x0000000000200040
                 0x0000000000000118 0x0000000000000118  R      0x8
  LOAD           0x0000000000000000 0x0000000000200000 0x0000000000200000
                 0x0000000000000158 0x0000000000000158  R      0x1000
  LOAD           0x0000000000000160 0x0000000000201160 0x0000000000201160
                 0x0000000000000014 0x0000000000000014  R E    0x1000
  LOAD           0x0000000000000174 0x0000000000202174 0x0000000000202174
                 0x0000000000000000 0x0000000000000001  RW     0x1000
  GNU_STACK      0x0000000000000000 0x0000000000000000 0x0000000000000000
                 0x0000000000000000 0x0000000000000000  RW     0x0
```

# Putting the user mode program in our ISO
In `runner/Cargo.toml`, add this to `[build-dependencies]`:
```toml
user_mode_program_0 = { path = "../user_mode_program_0", artifact = "bin", target = "x86_64-unknown-none" }
```
In `runner/build.rs`, add:
```rs
let user_mode_program_0_executable_file = env::var("CARGO_BIN_FILE_USER_MODE_PROGRAM_0").unwrap();
```
```rs
ensure_symlink(
    user_mode_program_0_executable_file,
    iso_dir.join("user_mode_program_0"),
)
.unwrap();
```

# Accessing the user mode ELF from our kernel
Limine's module request let's us ask Limine to load additional files into memory before booting our kernel. In `limine_requests.rs`, add:
```rs
pub const USER_MODE_PROGRAM_0_PATH: &CStr = c"/user_mode_program_0";
#[used]
#[unsafe(link_section = ".requests")]
pub static MODULE_REQUEST: ModuleRequest = ModuleRequest::new()
    .with_internal_modules(&[&InternalModule::new().with_path(USER_MODE_PROGRAM_0_PATH)]);
```

# Loading the ELF
Create a file `run_program_0.rs`:
```rs
pub fn run_program_0() { }
```
And in `init_bsp`, add
```rs
run_program_0();
```
before `hlt_loop();`.

In `run_program_0`, the first thing we need to do is get a `&File` from Limine:
```rs
// Parse the ELF
let module = MODULE_REQUEST
    .get_response()
    .unwrap()
    .modules()
    .iter()
    .find(|module| module.path() == USER_MODE_PROGRAM_0_PATH)
```
Next, we need to get a `&[u8]` to the file so we can parse it.
```rs
let ptr = NonNull::new(slice_from_raw_parts_mut(
    module.addr(),
    module.size() as usize,
))
.unwrap();
// Safety: Limine gives us a valid pointer and len
let elf_bytes = unsafe { ptr.as_ref() };
```
Next, we need to parse these bytes as an ELF. Conveniently, the `elf` crate exists to do this:
```toml
elf = { version = "0.8.0", default-features = false }
```
So let's parse the ELF, panicking if there is an error:
```rs
let elf = ElfBytes::<AnyEndian>::minimal_parse(elf_bytes).expect("ELF should be valid");
```
Before we can start mapping the ELF, we need to create a new *address space* for the user mode program. We are keeping all kernel memory in the higher half of virtual memory. We will use the lower half of virtual memory for user space. This way, we can transition from kernel mode to user mode without changing address spaces. Since all processes will have their own address space, they can all use the same lower half addresses without conflicts. We will just have to switch address spaces when switching programs.

**Warning**: we are making the kernel memory in the higher half accessible to all user address spaces. On CPUs affected by the [Meltdown vulnerability](https://en.wikipedia.org/wiki/Meltdown_(security_vulnerability)), user programs *could* in theory **read** memory that they are not supposed to be able to read. For example, if you type a password on a "Web Browser" app, a "Video Game" app *could* access that password in memory. To focus effort on writing fun code, this tutorial will not include mitigation for the Meltdown vulnerability. Just keep in mind that this tutorial's OS is not the most secure OS. If you are confident in your OS dev abilities, feel free to add a mitigation to your own OS.

To allocate physical frames for the user program, let's modify `physical_memory.rs`. Add a `MemoryType::UsedByUserMode` enum variant, and
```rs
impl PhysicalMemory {
    pub fn get_user_mode_program_frame_allocator(&mut self) -> PhysicalMemoryFrameAllocator<'_> {
        PhysicalMemoryFrameAllocator {
            physical_memory: self,
            memory_type: MemoryType::UsedByUserMode,
        }
    }
}
```
Now back in `run_program_0`, we can create the address space:
```rs
// Create a new address space for the user mode program
let memory = MEMORY.get().unwrap();
let mut physical_memory = memory.physical_memory.lock();
let mut user_l4 = memory.virtual_memory.lock().l4_mut().new_user(
    physical_memory
        .get_user_mode_program_frame_allocator()
        .allocate_4kib_frame()
        .unwrap(),
);
```

We will need to get the flags of each segment. The `elf` crate let's us get these through the `p_flags` field, which is a `u32`. If bit `0` is `1`, the `R` flags is present. If bit `1` is `1`, the `W` flag is present. If bit `2` is `1`, the `E` flag is present. Let's create a nice struct to parse these bits in a readable way using the `bitflags` crate:
```toml
bitflags = "2.9.4"
```
Create `elf_segment_flags.rs`:
```rs
use bitflags::bitflags;
use elf::segment::ProgramHeader;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct ElfSegmentFlags: u32 {
        const EXECUTABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        const READABLE = 1 << 2;

        // The source may set any bits
        const _ = !0;
    }
}

impl From<ProgramHeader> for ElfSegmentFlags {
    fn from(value: ProgramHeader) -> Self {
        Self::from_bits_retain(value.p_flags)
    }
}
```
Next we will need to actually load the segment. That is, we need to "place" the segment into requested space in the user program's virtual address space.

The cool thing about ELF files is that they minimize the amount of physical frames we have to allocate towards a program, and minimizes the amount of memory that we need to copy or zero. Consider the `LOAD` segments above, after we forced there to be a `RW` `LOAD` segment. 

The first segment (`R`) starts at `0x0000000000000000` in the ELF file and at `0x0000000000200000` in the program memory. Both addresses have the `0x0` offset from a 4 KiB page.

The next segment (`R E`) starts at `0x0000000000000160` in the ELF file and at `0x0000000000201160` in the program memory. Both addresses have a `0x160` offset from a 4 KiB page.  

The last segment (`RW`) starts at `0x0000000000000174` in the ELF file and at `0x0000000000202174` in the program memory. Both addresses have a `0x174` offset from a 4 KiB page.

The data inside the ELF (code and `static` variables) is compactly arranged. At the same time, all segments are spaced out from each other enough so that each segment will have its own 4 KiB pages, letting us assign write and execute permissions on each segment.

We can think of each segment in the ELF as `[u8]`. The program needs a `&` ref to `R` and `R E` segments, and a `&mut` ref to `RW` segments. 

If we wanted to only run 1 instance of the program, we could transfer ownership of the ELF memory to the user mode program. We can crate pages that point to the existing frames that Limine put the module in. Then we would not have to copy any memory. But remember, we would be transferring ownership of the Limine module, and letting the user mode program to modify it.

If we want to run multiple instances of the program, we can create pages that point to the read-only segments of the ELF, while creating new frames for `RW` segments and copying the data from the ELF to the new frames. This way, all read-only segments can be shared between all instances of the program, while still isolating each instance so it cannot corrupt the ELF for the other instances.

The program that we are going to spawn is similar to [PID 1 on Linux](https://medium.com/@boutnaru/the-linux-process-journey-pid-1-init-60765a069f17). We won't need to create multiple instances of it, so we can consume the Limine module instead of copying it. To make sure that we don't try to use the module after consuming it, we can store a global variable in the kernel:
```rs
static CONSUMED_USER_MODE_PROGRAM_0: AtomicBool = AtomicBool::new(false);
```
and then first thing inside the `run_program_0` fn:
```rs
let previously_consumed = CONSUMED_USER_MODE_PROGRAM_0.swap(false, Ordering::Relaxed);
assert!(!previously_consumed);
```

Now let's start loading! Quick instructions on how we are supposed to load a segment:
- Copy (or move) `p_filesz` bytes to virtual memory starting at `p_vaddr`
- Set following (`p_memsz` - `p_filesz`) bytes to `0`
- Use memory protection features to enforce the flags  

An important thing to note is that `p_memsz` can be > `p_filesz`. This is so that the ELF doesn't need to include a bunch of contiguous `0`s. The ELF loader (that's the code we will write) must fill in `0`s there. And technically  `p_memsz` can be so much greater than `p_filesz` that we have to allocate new pages in addition to the pages occupied by the ELF file.

To make things easier, we will only load "good" ELFs:
- All read-only sections must have `p_memsz` = `p_filesz`
- If there is a section with `p_filesz` > `0` and `p_memsz` > `p_filesz`, and zeroing the additional memory region should not mess up other segments

This way we can simplify our ELF consuming and loading logic:
- Map all pages (based on `p_vaddr` and `p_filesz`) in each segment to the physical frames of the ELF.
- If the last segment has `p_memsz` > `p_filesz`, we zero the memory after `p_filesz` up to the page boundary. If we still need more zero-initialized memory, we allocate new physical frames and completely zero them, and mark them as used by the user mode program.
- Mark all of the ELF's physical frames used by segments (which we can calculate based on `p_offset` and `p_filesz`) as owned by the user mode program.
- Mark all of the ELF's physical frames not used by segments as available.
- If the ELF's last frame is referenced, zero any remaining bytes in that frame.

We will only be using ELFs generated by Rust, so the ELFs will be valid. However, our loading code needs to be able to handle invalid and malicious ELFs as well.

Here is code that does the steps above
```rs
// Remove the module from physical memory map
// We will only be using 4 KiB pages because most ELFs will have segments only aligned to 4 KiB, and Limine only aligns the ELF to 4 KiB
let page_size = PageSize::_4KiB;
let module_physical_interval = {
    let start = VirtAddr::from_ptr(module.addr()).offset_mapped().as_u64();
    ie(
        start,
        (start + module.size()).next_multiple_of(page_size.byte_len_u64()),
    )
};
let _ = physical_memory.map_mut().cut(module_physical_interval);

// Map ELF segments
let mut range_to_zero: Option<Range<PhysAddr>> = None;
for segment in elf.segments().unwrap() {
    if ElfSegmentType::try_from(segment.p_type) != Ok(ElfSegmentType::Load) {
        continue;
    }

    // Make sure the segment is only referencing file memory contained within the ELF
    assert!(segment.p_offset + segment.p_filesz <= module.size());

    let start_page = Page::new(
        VirtAddr::new(segment.p_vaddr).align_down(page_size.byte_len_u64()),
        page_size,
    )
    .unwrap();
    let file_pages_len = if segment.p_filesz > 0 {
        (segment.p_vaddr + segment.p_filesz).div_ceil(page_size.byte_len_u64())
            - segment.p_vaddr / page_size.byte_len_u64()
    } else {
        0
    };
    let start_frame = Frame::new(
        (VirtAddr::from_ptr(module.addr()).offset_mapped() + segment.p_offset)
            .align_down(page_size.byte_len_u64()),
        page_size,
    )
    .unwrap();

    // Map the virtual memory to the ELF's frames
    let flags = ElfSegmentFlags::from(segment);
    let mut frame_allocator = physical_memory.get_user_mode_program_frame_allocator();
    let flags = ConfigurableFlags {
        pat_memory_type: PatMemoryType::WriteBack,
        writable: flags.contains(ElfSegmentFlags::WRITABLE),
        executable: flags.contains(ElfSegmentFlags::EXECUTABLE),
    };
    for i in 0..file_pages_len {
        let page = start_page.offset(i).unwrap();
        let frame = start_frame.offset(i).unwrap();
        unsafe { user_l4.map_page(page, frame, flags, &mut frame_allocator) }.unwrap();
    }

    // Mark the ELF's frames as used by user mode
    if file_pages_len > 0 {
        let interval = {
            let start = start_frame.start_addr().as_u64();
            ie(start, start + file_pages_len * page_size.byte_len_u64())
        };
        let _ = physical_memory.map_mut().cut(interval);
        physical_memory
            .map_mut()
            .insert_merge_touching_if_values_equal(interval, MemoryType::UsedByUserMode)
            .unwrap();
    }

    if segment.p_memsz > segment.p_filesz {
        if segment.p_filesz > 0 {
            // We need to zero any remaining bytes from that frame
            assert_eq!(
                range_to_zero, None,
                "there can only be up to 1 segment with p_memsz > p_filesz"
            );
            range_to_zero = Some({
                let start = VirtAddr::from_ptr(module.addr()).offset_mapped()
                    + segment.p_offset
                    + segment.p_filesz;
                start..start.align_up(page_size.byte_len_u64())
            });
        }

        // We need to allocate, zero, and map additional frames
        let extra_pages_len = (segment.p_vaddr + segment.p_memsz)
            .div_ceil(page_size.byte_len_u64())
            - (segment.p_vaddr + segment.p_filesz).div_ceil(page_size.byte_len_u64());
        let start_page = start_page.offset(file_pages_len).unwrap();
        for i in 0..extra_pages_len {
            let page = start_page.offset(i).unwrap();
            let frame = physical_memory
                .allocate_frame_with_type(page_size, MemoryType::UsedByUserMode)
                .unwrap();
            let frame_ptr =
                NonNull::new(frame.offset_mapped().start_addr().as_mut_ptr::<u8>()).unwrap();
            // Safety: we own the frame
            unsafe { frame_ptr.write_bytes(0, page_size.byte_len()) };
            let mut frame_allocator = physical_memory.get_user_mode_program_frame_allocator();
            unsafe { user_l4.map_page(page, frame, flags, &mut frame_allocator) }.unwrap();
        }
    }
}

// Map all non-referenced frames in the ELF as usable
// Currently all non-referenced frames are gaps in the map
// We just need to "fill the gaps" with usable
loop {
    let gap = physical_memory
        .map_mut()
        .gaps_trimmed(module_physical_interval)
        .next();
    if let Some(gap) = gap {
        physical_memory
            .map_mut()
            .insert_merge_touching_if_values_equal(gap, MemoryType::Usable)
            .unwrap();
    } else {
        break;
    }
}

// Zero the range we need to zero, if needed
if let Some(range_to_zero) = range_to_zero {
    let count = (range_to_zero.end - range_to_zero.start) as usize;
    let ptr = NonNull::new(range_to_zero.start.offset_mapped().as_mut_ptr::<u8>()).unwrap();
    // Safety: we now have exclusive access to the ELF bytes
    unsafe { ptr.write_bytes(0, count) };
}

// Zero the unused bytes of the last frame of the ELF module, if it's used
// For simplicity we will just uncoditionally zero it
{
    let start = module.addr().addr() + module.size() as usize;
    let ptr = NonNull::new(start as *mut u8).unwrap();
    let end = start.next_multiple_of(page_size.byte_len());
    let count = end - start;
    // Safety: we have exclusive accces to this memory
    unsafe { ptr.write_bytes(0, count) };
}
```
And in `physical_memory.rs`, add a way of getting the map:
```rs
impl PhysicalMemory {
    pub fn map_mut(&mut self) -> &mut NoditMap<u64, Interval<u64>, MemoryType> {
        &mut self.map
    }
}
```

# Allocating a stack
Similar to how Limine allocates a stack for our kernel, we need to allocate a stack for the user mode program.
```rs
// Allocate a stack
// Technically this stack placement could overlap with our ELF, but we will assume it won't
let rsp = LOWER_HALF_END;
{
    let stack_size: u64 = 64 * 0x400;
    // We are using 4 KiB pages because we need <2 MiB, but we could use any page size for the stack, as long as the stack size is a multiple of it
    let page_size = PageSize::_4KiB;
    let pages_len = stack_size.div_ceil(page_size.byte_len_u64());
    let start_page = Page::new(
        VirtAddr::new(rsp - pages_len * page_size.byte_len_u64()),
        page_size,
    )
    .unwrap();
    for i in 0..pages_len {
        let page = start_page.offset(i).unwrap();
        let frame = physical_memory
            .allocate_frame_with_type(page_size, MemoryType::UsedByUserMode)
            .unwrap();
        let flags = ConfigurableFlags {
            pat_memory_type: PatMemoryType::WriteBack,
            writable: true,
            executable: false,
        };
        let mut frame_allocator = physical_memory.get_user_mode_program_frame_allocator();
        unsafe { user_l4.map_page(page, frame, flags, &mut frame_allocator) }.unwrap()
    }
};
```

# Entering user mode
## Enabling `sysretq`
There are two ways we can enter user mode, `iret` and `sysretq`. There is no dedicated method to entering user mode for the first time. `iretq` is used to return from an interrupt. `sysretq` is used to return from a system call. Both methods require using an instruction as if we were just *returning back* to user mode, even though we are actually entering it for the first time. Because it's simpler, we will use `sysretq` to enter user mode. To enable it, we need to update the [`Efer`](https://wiki.osdev.org/CPU_Registers_x86-64#IA32_EFER) register. In `run_program_0`, add:
```rs
// Enable syscall in IA32_EFER
// https://shell-storm.org/x86doc/SYSCALL.html
// https://wiki.osdev.org/CPU_Registers_x86-64#IA32_EFER
unsafe {
    Efer::update(|flags| {
        *flags = flags.union(EferFlags::SYSTEM_CALL_EXTENSIONS);
    })
};
```

## Doing `sysretq`
Create `enter_user_mode.rs`:
```rs
use core::arch::asm;

use x86_64::registers::rflags::RFlags;

pub struct EnterUserModeInput {
    pub rip: u64,
    pub rsp: u64,
    pub rflags: RFlags,
}

/// # Safety
/// Does `sysretq`.
/// Make sure that you are not letting the user space program do things you don't want it to do.
/// You must enable system call extensions first.
pub unsafe fn enter_user_mode(EnterUserModeInput { rip, rsp, rflags }: EnterUserModeInput) -> ! {
    let rflags = rflags.bits();
    unsafe {
        // Note that we do `sysretq` and not `sysret` because if we just do `sysret` that could be compiled into a `sysretl`, which is for 32-bit compatibility mode and can mess things up.
        asm!("\
            mov rsp, {}
            sysretq",
            in(reg) rsp,
            in("rcx") rip,
            in("r11") rflags,
            // The user space program can only "return" with a `syscall`, which will jump to the syscall handler
            options(noreturn)
        );
    }
}
```

## Calling `enter_user_mode`
In `run_program_0`, before the `if` block that does `drop(elf);`, add
```rs
// Parse the entry point before we drop `elf`
let entry_point = NonZero::new(elf.ehdr.e_entry).expect("entry point should be defined in ELF");
```
Next we need to switch to the user mode program's address space:
```rs
// Switch to the user address space
// Safety: we can still reference kernel memory
unsafe { user_l4.switch_to(memory.new_kernel_cr3_flags) };
```
And finally, we use the entry point and the top of the stack we mapped to call `enter_user_mode`:
```rs
let input = EnterUserModeInput {
    rflags: RFlags::empty(),
    rip: entry_point.get(),
    rsp: rsp,
};
unsafe { enter_user_mode(input) };
```

# Checking if it worked
Our user mode program does not log any messages to confirm that it's running (yet). Currently, it does a spin loop. We have two signs that it's working as expected:
- The kernel did not panic, or have an exception, or triple fault
- The CPU usage should show 1 CPU as using 100% usage

For now, to be more sure that it's working, let's make the user mode program cause a page fault:
```rs
unsafe {
    (0xABCDEF as *mut u8).read_volatile();
}
```
Now we should see
```rs
Page fault! Stack frame: InterruptStackFrame {
    instruction_pointer: VirtAddr(
        0x201600,
    ),
    code_segment: SegmentSelector {
        index: 2,
        rpl: Ring3,
    },
    cpu_flags: RFlags(
        RESUME_FLAG | AUXILIARY_CARRY_FLAG | PARITY_FLAG | 0x2,
    ),
    stack_pointer: VirtAddr(
        0x7fffffffffc8,
    ),
    stack_segment: SegmentSelector {
        index: 1,
        rpl: Ring0,
    },
}. Error code: PageFaultErrorCode(
    USER_MODE,
). Accessed address: VirtAddr(0xabcdef).
```
And this time, the error code contains the `USER_MODE` flag! This confirms it. We successfully loaded a user mode program and executed it.

# Learn More
- <https://en.wikipedia.org/wiki/Kernel_(operating_system)>
- <https://en.wikipedia.org/wiki/Executable>
- <https://en.wikipedia.org/wiki/Executable_and_Linkable_Format>
- <https://offlinemark.com/kpti-the-virtual-memory-101-fact-thats-no-longer-true/>
- <https://en.wikipedia.org/wiki/Kernel_page-table_isolation>
- <https://medium.com/@boutnaru/the-linux-process-journey-pid-1-init-60765a069f17>
- <https://nfil.dev/kernel/rust/coding/rust-kernel-to-userspace-and-back/>. Warning: syscall handler implementation is unsound.
- <https://wiki.osdev.org/Getting_to_Ring_3>
- <https://en.wikipedia.org/wiki/Protection_ring>
- <https://www.felixcloutier.com/x86/iret:iretd:iretq>
- <https://www.felixcloutier.com/x86/sysret>
- <https://wiki.osdev.org/CPU_Registers_x86-64#IA32_EFER>
