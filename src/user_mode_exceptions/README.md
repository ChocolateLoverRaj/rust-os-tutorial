# Handling user mode exception
User mode programs can cause various exceptions. Common ones are page faults and divide by zero exceptions. Different operating systems handle exceptions caused by user code differently. 

On Linux, programs can "catch" exceptions by registering signal handlers. They can then recover from these exceptions and continue to execute the thread.

Usually, writing signal handlers is not something you do when writing programs, besides handling `SIGINT`, which is sent by doing `Ctrl + C` on a keyboard.

Whether you should even have signal handlers for exceptions is debatable. If an exception happens, other memory might be corrupted or invalid, which can cause the signal handler to also behave unexpectedly.

In seL4, when an exception happens, the kernel can send a message to a separate thread belonging to the same process as the thread the caused the exception. That separate thread can then recover from the error and resume execution of the original thread if it wants.

We will be designing our OS to run programs written in Rust. Rust does not use exception handlers. Rust's strategy is to simply not cause exceptions, by ensuring that the compiled code cannot cause exceptions. Rust's memory safety protects against page faults. Rust checks numbers before dividing, and panics if a division would result in attempting to divide by 0. The only exception handler that Rust uses is a page fault handler, which is to print a nice message in case of a stack overflow, and differentiate it from a non-stack-overflow page fault.

For now, we will not need to handle any exceptions. It would be nice to have pretty page fault error messages, but we can live without that for now. So for our initial implementation, if a thread causes an exception, we can just change the state of that thread to "ended" and log a warning.
