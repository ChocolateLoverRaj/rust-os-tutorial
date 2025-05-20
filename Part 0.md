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

# The `kernel` and `runner` folder
For now, the kernel is our operating system. We will also have a `runner` folder which will have programs that run our operating system in a virtual machine.
