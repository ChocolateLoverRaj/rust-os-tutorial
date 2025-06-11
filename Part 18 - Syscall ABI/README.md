In the previous part, we implemented a way to use `[u64; 7]` for input and output in syscalls. Now, we have to decide how the inputs and outputs are going to be formatted. Are we going to use the first `u64` as the syscall number? Are we going to return a single `u64` as an output like Linux does? Are we going to pass a pointer to the actual input? Are the syscall numbers going to start from 0 and go up incrementally as we add more syscalls? Will we be adding options to existing syscalls in the future?

Now that our OS is starting to become a multiple binary OS, we should consider compatibility between different versions of the kernel and the user mode programs. Do we want future versions of the kernel to be compatible with old programs? Do we want new programs to be compatible with old versions of the kernel? Syscalls are *the way* that programs communicate with the kernel. The format of the syscall inputs and outputs is an [ABI](https://en.wikipedia.org/wiki/Application_binary_interface). We should consider possible compatibility issues with different methods.

Linus Torvalds emphasizes an important objective of Linux: to not break user space. No Linux kernel update should be made if it breaks existing user mode programs from working. If it does, then it should be reverted. You can decide if this is something you care about for your own OS. Linux is the most important and depended on OS for most computers in the world. Your hobby OS does not have to have the same goals and rules as Linux. At the same time, because Linux is so good, successful, and made by experienced people, Linux is a great example for what to do when making an OS in general.

In this tutorial, we will use a [UUID](https://en.wikipedia.org/wiki/Universally_unique_identifier) to identify a syscall. That's 2 x `u64` used in our input, so we'll only have `[u64; 5]` for our inputs. For the outputs, we will still have the `[u64; 7]`, since we don't need to return a UUID. **Warning: this is probably not good advice for making a high-performance OS. Linux, for example, only uses 1 `u64` for the syscall number, and practically you only need a `u16` or `u32` for a syscall number.** I chose to use a 128-bit UUID because it's easy to generate and guarantees that even if people have their own, very customized kernels, they can generate their own random UUIDs without worrying about a conflict at all. 

Our OS will have core syscalls and optional syscalls. The core syscalls are:
- Exists, to check if a syscall with a certain UUID is supported by the kernel
- Exit, to exit the current program

There will be no guarantees about if optional syscalls exist. Some variations of the kernel might have some optional syscalls but not others. In practice, since most people will make their own personal OS, or just follow this tutorial exactly, we will know exactly which syscalls are available since we'll be writing the kernel side and user side of syscalls.

And, because this is Rust, we will not be doing things like returning `-1` for errors. We're not going to use null pointers as `None` or `Err`. We're going to use Rust types, with enums, including `Option` and `Result`, and anything that works with `serde`.

# Common crate
We're going to be sharing a lot of data types between the kernel and user programs. Let's create a common crate for them. In `Cargo.toml`, add the workspace member `"common"`. Then create `common/Cargo.toml`:
```toml
[package]
name = "common"
version = "0.1.0"
edition = "2024"
publish = false
```
and `common/src/lib.rs`:
```rs
#![no_std]
```
Also, let's add the `uuid` and `serde` crates to `common` for using UUIDs:
```toml
serde = { version = "1.0.219", default-features = false }
uuid = { version = "1.17.0", default-features = false, features = ["serde"] }
```
For each syscall, we need to associate the UUID, input and output types. For this, let's create `syscall.rs`:
```rs
pub trait Syscall {
    const UUID: Uuid;

    type Input: Serialize + DeserializeOwned;
    type Output: Serialize + DeserializeOwned;
}
```
Then in `lib.rs` add:
```rs
mod syscall;
pub use syscall::*;
```

Then let's define types for the first syscall that we'll implement: `syscall_exists.rs`:
```rs
use uuid::{Uuid, uuid};

use crate::Syscall;

pub struct SyscallExists;
impl Syscall for SyscallExists {
    const UUID: Uuid = uuid!("c44fcf37-dde4-4c0b-a15f-85e995b64e96");
    type Input = Uuid;
    type Output = bool;
}
```
The input is simply a `Uuid`, the UUID of the syscall to check if it exists, and the output is simply a `bool`. This syscall can't fail, and there is no invalid input.

For the actual serialization and deserialization, we can choose which format to use 

# Learn More
- https://en.wikipedia.org/wiki/Application_binary_interface
- https://en.wikipedia.org/wiki/Universally_unique_identifier
