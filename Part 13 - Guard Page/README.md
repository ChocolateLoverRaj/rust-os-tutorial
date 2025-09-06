# Guard Page
As mentioned in the part about page fault handling, our kernel currently has no protections against stack overflows.

Let's purposely create a page fault:
```rs
fn stack_overflow() {
    stack_overflow();
}
stack_overflow();
```
Since we have no guard page, running this code could result in *anything*. When I ran this code, I got an "Invalid Opcode" fault. But we know that the underlying cause was not an invalid opcode.

## Allocating and mapping memory for a stack with a guarded page

## Switching stacks

## Interrupt Stack Table (IST)

## Logging stack overflows
