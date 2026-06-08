## How to structure files / crates for multiple architectures?

### Idea: single crate with `#[cfg]`s
You can create modules that activate different functions based on different architectures. You can have target-specific dependencies in `Cargo.toml`.

Pros:
- Simple
- Pretty easy to add architectures
- Less confusing

Cons:
- Kernel gets coupled with architecture-specific things.
- Might be messy

### Idea: different kernel for each arch
Would result in a lot of duplicate code but would make it easier to experiment with concepts without having to implement the concept on each target.

Pros:
- Simple
- Good for learning new architectures

Cons:
- Gets more annoying as code base increases

### Idea: kernel as library crate, each arch as a kernel crate
Move as much code as possible into the library crate, which the kernel crates can call providing methods to do architecture-specific things.

Pros:
- Kernel library is clean
- Easy to add and drop support for different architectures
- Dependencies are clean without a ton of `#[cfg]`s

Cons:
- Need to use a ton of traits and probably generic types
- Need to generalize every function to support all architectures we want

## The plan
Start with a different kernel for each arch to learn the arch, and then create a kernel that combines the architectures either as a single kernel crate or as a kernel library + architecture specific crates.
