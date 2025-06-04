# What is user mode?
So far, our operating system has been just a kernel. In an OS, the kernel is code that runs with full permissions. The kernel can modify page tables, access any memory, and access any I/O port.

Part of what an operating system is expected to do is run arbitrary, untrusted programs without letting those programs do bad things such as cause a kernel panic, triple fault, and access things it's not allowed to. To do this, CPUs have a thing called a privilege level. In x86_64, there are two privilege levels used in modern day operating systems. Ring 0, which is kernel mode, and Ring 3, which is user mode. 

User mode is capable of running programs with restricted permissions, so that code running in user mode cannot mess up your computer, kernel, or other programs (as long as the kernel doesn't have security vulnerabilities that let user mode code bypass restrictions). User mode is essential for running code that you don't fully trust. User mode also helps contain the damage caused by buggy code, making it so that at worst, a program will just crash itself and not crash the kernel or other programs. Even if you're planning on only running your own code that you trust on your OS, you should still run as much of your code as you practically can in user mode.

# Programs and executable file formats
You should be familiar with programs. Whether it's a graphical app, a command line tool, or a systemd service, all apps are made up of at least 1 executable file. Before we can start *executing* a program in user mode, we need to load it into memory. At minimum, we need to load the executable parts of a program (which contains CPU instructions), immutable global variables (`const`), mutable global variables (`static`), and memory for the stack.

There are many different file formats that store information about these memory regions. For our OS, we will be using the ELF format for our programs. This format is widely used in operating systems, including Linux, FreeBSD, and [RedoxOS](https://www.redox-os.org/). It is also the format that Rust outputs when building code for the `x86_64-unknown-none` target.

# Creating a program
In your workspace's `Cargo.toml` file, add a workspace member `"user_mode_program"`. Then create a folder `user_mode_program` with `Cargo.toml`:
```toml
[package]
name = "user_mode_program"
version = "0.1.0"
edition = "2024"
publish = false

[[bin]]
name = "user_mode_program"
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
unsafe extern "C" fn entry_point() -> ! {
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
cargo build --package user_mode_program --target x86_64-unknown-none
```
Now we can look at the generated code:
```bash
objdump -d target/x86_64-unknown-none/debug/user_mode_program
```
```
target/x86_64-unknown-none/debug/user_mode_program:     file format elf64-x86-64


Disassembly of section .text:

0000000000201120 <entry_point>:
  201120:       eb 00                   jmp    201122 <entry_point+0x2>
  201122:       f3 90                   pause
  201124:       eb fc                   jmp    201122 <entry_point+0x2>
```
We can see that our `entry_point` got compiled into 3 instructions, which make the forever loop.

Now let's look at the ELF file info needed by our kernel to load and run the program:
```bash
readelf --file-header --segments target/x86_64-unknown-none/debug/user_mode_program
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
  Start of section headers:          4496 (bytes into file)
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

Now let's look at the program headers, aka segments. Our kernel will only need to process the segments that are type `LOAD`. Looking at the flags, we can see that our ELF has two segments. One that is read-only and one that is read and execute. Once we use `static` global variables in our program, there will be another `LOAD` segment with read-write flags. 

# Putting the user mode program in our ISO
In `runner/Cargo.toml`, add this to `[build-dependencies]`:
```toml
user_mode_program = { path = "../user_mode_program", artifact = "bin", target = "x86_64-unknown-none" }
```
In `runner/build.rs`, add:
```rs
let user_mode_program_executable_file = env::var("CARGO_BIN_FILE_USER_MODE_PROGRAM").unwrap();
```
```rs
ensure_symlink(
    user_mode_program_executable_file,
    iso_dir.join("user_mode_program"),
)
.unwrap();
```

# Accessing the user mode ELF from our kernel
Limine's module request let's us ask Limine to load additional files into memory before booting our kernel. Create `user_mode_program_path.rs`:
```rs
use core::ffi::CStr;

pub const USER_MODE_PROGRAM_PATH: &CStr = c"/user_mode_program";
```
And in `limine_requests.rs`, add:
```rs
#[used]
#[unsafe(link_section = ".requests")]
pub static MODULE_REQUEST: ModuleRequest = ModuleRequest::new()
    .with_internal_modules(&[&InternalModule::new().with_path(USER_MODE_PROGRAM_PATH)]);
```

# Loading the ELF
TODO: get `&[u8]` from Limine request. 

We will use the `elf` crate to parse the ELF format:
```toml
elf = { version = "0.8.0", default-features = false }
```

TODO: actual loading part

# Entering user mode
## Updating the GDT
In `gdt.rs`, add the following fields to the `Gdt` struct:
```rs
user_code_selector: SegmentSelector,
user_data_selector: SegmentSelector,
```
And then update the GDT-creating function to be:
```rs
|| {
    let mut gdt = GlobalDescriptorTable::new();
    // Changing the order of these could mess things up!
    let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
    let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
    let tss_selector = gdt.append(Descriptor::tss_segment(tss));
    let user_data_selector = gdt.append(Descriptor::user_data_segment());
    let user_code_selector = gdt.append(Descriptor::user_code_segment());
    Gdt {
        gdt,
        kernel_code_selector,
        kernel_data_selector,
        tss_selector,
        user_code_selector,
        user_data_selector,
    }
}
```
Then, after loading the GDT,

## Enabling `sysretq`
There are two ways we can enter user mode, `iret` and `sysretq`. There is no dedicated method to entering user mode for the first time. `iretq` is used to return from an interrupt. `sysretq` is used to return from a system call. Both methods require using an instruction as if we were just *returning back* to user mode, even though we are actually entering it for the first time. Because it's simpler, we will use `sysretq` to enter user mode. To enable it, we need to update the [`Efer`](https://wiki.osdev.org/CPU_Registers_x86-64#IA32_EFER) register:
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

use x86_64::{VirtAddr, registers::rflags::RFlags};

pub struct EnterUserModeInput {
    pub rip: VirtAddr,
    // We don't use `VirtAddr` for rsp because 0x800000000000 is a valid rsp value, which is not a valid VirtAddr
    pub rsp: u64,
    pub rflags: RFlags,
}

/// # Safety
/// Does sysret.
/// Make sure that you are not letting the user space program do things you don't want it to do.
/// You must enable system call extensions first.
pub unsafe fn enter_user_mode(EnterUserModeInput { rip, rsp, rflags }: EnterUserModeInput) -> ! {
    let rip = rip.as_u64();
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
TODO

## Did work?
Let's use GDB to check:
```bash
gdb runner/out_dir/iso_root/user_mode_program
```
In GDB, run
```
target remote :1234
```
It should show this:
```rs
Remote debugging using :1234
user_mode_program::entry_point () at user_mode_program/src/main.rs:14
14              core::hint::spin_loop();
```
So our kernel successfully ran a user mode program! 

# Learn More
- https://en.wikipedia.org/wiki/Kernel_(operating_system)
- https://en.wikipedia.org/wiki/Executable
- https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
- https://nfil.dev/kernel/rust/coding/rust-kernel-to-userspace-and-back/. Warning: syscall handler implementation is unsound.
- https://wiki.osdev.org/Getting_to_Ring_3
- https://en.wikipedia.org/wiki/Protection_ring
- https://www.felixcloutier.com/x86/iret:iretd:iretq
- https://www.felixcloutier.com/x86/sysret
- https://wiki.osdev.org/CPU_Registers_x86-64#IA32_EFER
