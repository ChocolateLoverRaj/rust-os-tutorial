# Booting our kernel with Limine
We now have a runner which launches Limine in a VM. Now let's write the kernel that's in Limine's boot menu

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

