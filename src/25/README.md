# Our async data structures
In our OS, we will be using the memory condition (similar to `futex`) based method of waiting. An example of a wait syscall:
```rs
struct Condition {
  address: usize,
  wait_while_value_is: usize,
}
```
where the thread will pass a `&[Condition]`.

## Data stream from kernel to user
One use case is for the kernel to send PS/2 keyboard data to a user-mode program. It will place the raw data (`[u8]`) in a ring buffer (accessible to user mode), with two `AtomicUsize`s to keep track of the read count and write count.

### Initialization
A `[u8]` (can have uninitialized data, but keep in mind to not leak data to user mode) is allocated for the ring buffer, with two `AtomicUsize`s initialized to `0`.

### Kernel logic
On a PS/2 interrupt:
- Read the byte received
- Read read and write count
- If there is enough space in the ring buffer, place the keyboard byte into the ring buffer
- Update the write count
- `notify` (check if this write count update should wake up aa thread)

### User logic
To read from the stream
- Read read and write count
- If there are unread events, you now have a `(&[u8], &[u8])` from the ring buffer to reference / process.
- Once you process the memory, you update the read count and your reference is invalid
- If there are no unread events, you can wait for
```rs
Condition {
  address: address_of_write_count,
  wait_while_value_is: last_read_read_count,
}
```

## Mutex within a process
You can use a single `AtomicBool` (or, if the `wait` syscall only works with `usize`, you can just use an `AtomicUsize` with values `0` or `1`). `0` means that the mutex is not locked, and `1` means the mutex is locked.

### Acquiring the mutex
- Atomic swap to `true`
- If the previous value was `false`, you acquired the lock
- If the previous value was `true`, then you need to wait until it changes to `false` to acquire the lock
```rs
Condition {
  address: address_of_atomic_usize,
  wait_while_value_is: 1,
}
```

### Releasing the mutex
- Store to `false`
- `notify` syscall

## Mutex with priority inheritance
Usually, the kernel has to have a built-in priority inheritance feature to implement mutexes with priority inheritance in user code. So without implementing priority inheritance in our kernel, it will be messy.

In this method, we will assume that the `below` task of all threads that might acquire the lock is free to use for priority inheritance reasons.

Then we can assume that a task id is a `NonZero<usize>`. We can store a `Option<NonZero<usize>>` in the `AtomicUsize`. `None` can mean that the lock is not held by anything. `Some` can store the task id that is holding the lock. Now, when you want to wait for the lock, you can set the `below` to the id of the task that is holding the thread, and then call the `wait` syscall:
```rs
Condition {
  address: address_of_atomic_usize,
  wait_while_value_is: task_id,
}
```

Note that this solution can only priority boost a single other task. If, for some reason, a thread wanted to acquire two different locks, each held by two different tasks, then this simple solution won't be enough. Also, let's say tasks `1` and `2` are holding the locks we want. Then which task should have a higher priority relative to the other task? `1` or `2`? In our OS, the main reason we will want to use mutexes is for locking the global allocator. In this case, the only condition we'll need to wait on to acquire the lock to the global allocator is the atomic number for if the global allocator is locked, and we won't have to worry about priority boosting multiple threads.

We might not actually need priority inheritance. We might be able to adjust our code to avoid the problem of priority inheritance without relying on priority inheritance:
- https://lwn.net/Articles/178253/
- https://web.archive.org/web/20070706071207/http://www.linuxdevices.com/articles/AT7168794919.html


## Fast mutex
In Linux, `futex` stands for "fast mutex". "fast" means that in many cases, no syscall or kernel assistance is needed to lock or unlock a mutex. 

When acquiring a lock that is not held, all we need to do is a single atomic swap (which is fast). When acquiring a lock that is held by another thread, we need to call the `wait` syscall (which is slow).

When releasing a lock that is has no waiters (threads trying to acquire the lock, which are waiting for the lock to be released), all we need to do is a single atomic swap (which is fast). When releasing a lock that has waiters, we need to do an atomic swap, and then call the `notify` syscall (which is slow).

Fast mutexes need a way of knowing whether there are waiters when releasing the lock. This way, the thread releasing the thread can know whether to release it with the "fast path" or the "slow path". To keep track of this, we can use a separate `AtomicUsize` to keep track of how many waiters there are.

### Acquiring
- `waiter_count.fetch_add(1, Ordering::Relaxed)`
- `is_locked.swap(true, Ordering::Acquire)`
- If the previous value was `false`, that means the lock was acquired using the fast path
- If the previous value was `true`, wait for
```rs
Condition {
  address: address_of_atomic_usize,
  wait_while_value_is: 1,
}
```  

### Releasing
- `is_locked.store(false, Ordering::Release)`
- `waiter_count.fetch_sub(1, Ordering::Relaxed)`
- If the new waiter count is 0, that means the lock was released using the fast path
- If the new waiter count is >0, call the `notify` syscall (slow path)
