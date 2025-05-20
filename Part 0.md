# Introduction
Do you want to write your own operating system, from hello world to automatic updates? Do you like [Linux](https://www.kernel.org/category/about.html) and [Redox OS](https://www.redox-os.org/), but want to make your own operating system from the ground up, the way you want? Do you want to write your OS in [Rust](https://www.rust-lang.org/)? Then this tutorial is for you!

# Who this is for
You don't need to know Rust but you need to be able to learn. This tutorial will not teach you Rust, but it will provide links to learn if you don't know.

# Setting up the Development Environment
If you are using [NixOS](https://nixos.org/), then most of this will be very easy for you.

You will need...
- [Rust](https://www.rust-lang.org/) installed.
- A code editor. This is entirely your preference. I would recommend [Vscodium](https://vscodium.com/) with [rust-analyzer](https://open-vsx.org/extension/rust-lang/rust-analyzer), or [Zed](https://zed.dev/).
- [Git](https://git-scm.com/)

If you are using NixOS, you can just run `nix develop` in the directory containing `flake.nix`. I recommend using [direnv](https://direnv.net/) and [nix-direnv](https://github.com/nix-community/nix-direnv) so you can simply run `direnv allow` once and then your development environment will be set up automatically every time you enter the folder. If you're using Vscodium I would also recommend using the [direnv extension](https://open-vsx.org/extension/mkhl/direnv).

# Boot Loaders
The first thing we need to do is give our operating system control of the computer.

The entry-point of an operating system is an executable file. When a computer turns on, the first thing it runs is the firmware. Modern computers have [UEFI](https://en.wikipedia.org/wiki/UEFI) firmware, and very old computers have [BIOS](https://en.wikipedia.org/wiki/BIOS). The firmware looks for operating system executable files in various locations, including internal [SSDs](https://en.wikipedia.org/wiki/Solid-state_drive) and [HDDs](https://en.wikipedia.org/wiki/Hard_disk_drive), as well as external locations such as USB disks or servers on the network.

The protocol for giving control of the computer to an operating system is different for BIOS and UEFI. Working with BIOS and UEFI can be very complicated. Operating systems have [boot loaders](https://en.wikipedia.org/wiki/Bootloader) which go between the firmware and the actual operating system's entry point. The firmware gives control to the boot loader. Then the bootloader can do its own stuff, and eventually looks for the operating system entry point and gives control to it.

There are many boot loaders and boot loader protocols. A boot loader protocol basically states "this bootloader will boot operating systems in this way", and specific boot loader implementations can implement common boot protocols. Some examples of boot loader protocols are:
- [GRUB](https://en.wikipedia.org/wiki/GNU_GRUB)
- [Multiboot](https://en.wikipedia.org/wiki/Multiboot_specification)
- The Linux Boot Protocol
- [Limine](https://github.com/limine-bootloader/limine/blob/v9.x/PROTOCOL.md)
- [The Rust OSDev Bootloader](https://github.com/rust-osdev/bootloader), written for https://os.phil-opp.com/

In this tutorial, we will use Limine, because it is modern, simple, and makes writing an OS easy for us.

# Limine
By default, Limine simply calls an entry function in our operating system. We can ask Limine to do more things for us, such as setting up other CPUs to run our operating system's code on. Before calling our entry function, Limine goes through our executable file, checking for special data which are called Limine *requests*. Limine then does the set-up that the request asked it to do, and fills that area in the executable's memory with its *response*.

It's important to know that Limine requests and responses are not like HTTP requests and responses, where the client's code and server's code is running at the same time. All Limine requests get processed *before* our OS starts, and once our OS starts, Limine is not running anymore, and all Limine responses are loaded in memory, which our OS can access.

Our initial code will be based off of [limine-rust-template](https://github.com/jasondyoungberg/limine-rust-template). We will only be targeting `x86_64`.

# The `rust-toolchain.toml` file
Writing an operating system in Rust requires using nightly features, so we will specify a nightly toolchain. Our target platform is `x86_64-unknown-none`, which is for bare metal `x86_64`. So create a `rust-toolchain.toml` file:
```toml
[toolchain]
channel = "nightly-2025-05-19"
targets = ["x86_64-unknown-none"]
components = ["rust-src"]
```

# The `kernel` and `runner`
We will have two Rust projects: the kernel, which is our actual operating system, and the runner, which will have programs that run our operating system in a virtual machine.

Create a file `Cargo.toml`:
```toml
[workspace]
resolver = "3"
members = ["runner", "kernel"]
default-members = ["runner"]
```
The
```toml
default-members = ["runner"]
```
tells cargo that when we run `cargo run`, we want to run the binary in the `runner` project.

Create a file `kernel/Cargo.toml`:
```toml
[package]
name = "kernel"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
limine = "0.4"
x86_64 = "0.15.2"

[[bin]]
name = "kernel"
test = false
bench = false
```

Here we have two dependencies. `limine` is the Rust library to declare Limine requests and read responses. `x86_64` contains many useful functions for bare metal `x86_64` programming.

Now it's time to actually write the operating system's code! Create a file `kernel/src/main.rs`. In the top, add
```rs
#![no_std]
#![no_main]
```
This tells rust to not use the `std` part of the standard library and to not have a normal `main` function.

Next it's time for the special data to be read by Limine, as mentioned earlier:
```rs
/// Sets the base revision to the latest revision supported by the crate.
/// See specification for further info.
/// Be sure to mark all limine requests with #[used], otherwise they may be removed by the compiler.
#[used]
// The .requests section allows limine to find the requests faster and more safely.
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

/// Define the stand and end markers for Limine requests.
#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();
```

Don't worry about understanding the details of the code. What you need to know is that the `link_section`s place the `static` variables in a location that Limine reads, and the above code has 1 request, which is the base revision request. This request is to tell Limine what version of the Limine protocol we want.

Next we write our entry point function
```rs
#[unsafe(no_mangle)]
unsafe extern "C" fn main() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());
    halt_loop();
}
```
The `#[unsafe(no_mangle)]` makes sure that the compiler doesn't rename the `main` function to something else, since we need the entry point function to have a consistent name.

We mark the function as `unsafe` to reduce the chance of accidentally calling our `main` function from our own code.

We use `extern "C"` because Limine will call our function using the C calling convention.

First we check `BASE_REVISION` and make sure that it was set to 0 using the `is_supported` function. This way, we know that Limine booted our kernel using the protocol version that we expect.

Next, we do nothing. Instead of using `loop {}` to do nothing, we use call `halt_loop`:
```rs
fn halt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
```
We do the [`hlt`](https://www.felixcloutier.com/x86/hlt) instruction to tell the CPU to stop. The CPU isn't guaranteed to stop forever, and it might resume doing stuff and execute the next instruction. That's why we have a forever loop in which we call `hlt`.

But we also have to add a panic handler:
```rs
#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    halt_loop();
}
```
When using `std`, Rust already includes a panic handler which prints a nice message. However, since we are writing Rust for bare metal, we need to specify a function which gets called if our kernel panics. Later, we can also print a pretty message with the panic error, but for now, we just call `halt_loop`.

To make our kernel's executable file compatible with Limine, we need to add a linker file (`kernel/linker-x86_64.ld`). An important part to note is `ENTRY(main)`, where `main` is referencing the `main` function in our code. If you want, you can call the entry point function something else, such as `kernel_main` or `entry_point`, as long as you update the function in `main.rs` as well as the linker file.

