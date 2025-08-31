# Scheduling

## Goals
### Prioritization
Threads should always be explicitly prioritized. On a higher level, if you do Alt-Tab to switch windows, the Alt-Tab should never lag because of an app or background program hogging the CPU. A CPU running a high priority task should be interrupted as little as possible by things caused by lower priority tasks. 

### Utilization
All CPU cores should always be used if there are many tasks that can be run on all CPU cores.

## Use cases
### Spawning the initial thread
The kernel can spawn the initial thread

### Threads spawn new threads
Threads can spawn more threads

### The kernel can destroy threads
- Threads can crash and get destroyed by the kernel

### Threads can gracefully exit
- Threads can gracefully exit

### Allow threads to wake each other up

## State Structures

### Threads (THREAD_VEC)
- Grows proportional to number of threads
- `Vec`
- To grow / shrink this vec, we will have to put the vec inside a mutex, or, to improve performance, have some fancy mechanism to add threads.
- Earlier threads (threads with a smaller position) are higher priority
- Each thread has interior mutability based on a mutex. This mutex is always non-blocking. Nothing ever waits on the mutex.
```rs
struct ThreadVecItem {
    /// Read-only
    id: ThreadId,
    /// At any time, a CPU can compare and exchange from `None` to `Some` with `Ordering::Acquire`
    /// The CPU that has the lock can set this to `None` with `Ordering::Release`
    locked_by: Atomic<Option<ThreadId>>,
    /// At any time, a CPU can store `true` with `Ordering::Relaxed`
    is_destroying: AtomicBool,
    /// The CPU that has the lock can read and write to this
    /// This contains information about if the thread is running or waiting, and the required data
    thread_state: ThreadState,
}
```

### CPU current thread (CPU_CURRENT_THREAD_ARRAY)
- Array which has a length equal to the number of CPUs
- Basically an atomic `Option` which contains which thread the CPU is running

### CPU switch to slot (CPU_SWITCH_ARRAY)
- Array which has a length equal to the number of CPUs
- Basically an atomic `Option` which contains the thread that the CPU should switch to

### Events (EVENTS_MAP)
- Some sort of `Map`
- The key is the event id (a `u64`)
- The value is an `AtomicU64` that represents this enum:
    - `Pending` - this event already happened, but no thread asked to wait for it
    - `NotPending(Option<ThreadId>)` - this event is not pending, and wake up this thread when it does happen

## IPIs
- SWITCH_THREAD_IPI
- DESTROY_THREAD_IPI

## Logic

### Spawning a thread
- Acquire a lock to the threads
- `Vec::push` the new thread
- Call ASSIGN_THREAD

### Destroying a thread
- If you are running the thread you want to destroy
    - Call DESTROY_THREAD_FN
    - Call RUN_THREAD_FN
- Else
    - Find the thread in the THREAD_VEC
    - Store `is_destroying` to `true`
    - Try to acquire a lock to the thread
    - If `Ok`
        - TODO: Clean up resources maybe
        - Acquire a lock to the threads
        - Remove the item from the threads
    - If `Err(thread_id)`
        - Send a DESTROY_THREAD_IPI to `thread_id`

### Assigning a thread to a CPU (ASSIGN_THREAD)
- If you are not running a thread, you can initially claim it, put it in the running state, and run it
- Loop
    - Loop through the CPU_CURRENT_THREAD_ARRAY to find the CPU running the lowest priority thread. When considering the running thread, choose the thread with the highest priority out of that CPU's CPU_CURRENT_THREAD_ARRAY and CPU_SWITCH_ARRAY slot
    - If the CPU is running a lower priority thread than the spawned thread
        - Compare and exchange the CPU's slot in the CPU_SWITCH_ARRAY from the value read to `Some(thread_id)`
            - If `Ok`
                - If that CPU is you
                    - Run that task by calling SWITCH_THREAD_FN
                - Else
                    - Send that CPU a SWITCH_THREAD_IPI
            - If `Err(thread_id)`
                - Re-calculate the CPU running the lowest priority thread
    - Else
        - Go back to running your thread

### Running threads for the first time 
- Call RUN_THREAD_FN

### RUN_THREAD_FN
- Loop through every item in the threads vec
    - Try to acquire the thread mutex
    - If the thread mutex is held by a different CPU
        - Go to the next item in the threads vec
    - If you acquire the thread mutex
        - Check if the thread is ready to be run
        - If the thread is ready to be run:
            - Switch the task state to running and run the thread.
        - If not
            - Go to the next item in the threads vec
- If no tasks need to be run, just sleep (`hlt`)

### SWITCH_THREAD_IPI handler
- Call SWITCH_THREAD_FN

### SWITCH_THREAD_FN
- Swap your slot in CPU_SWITCH_ARRAY with `None`
- If the thread you are supposed to switch to is `None`
    - `unreachable!()`
- Try to acquire the thread mutex of the thread you are supposed to switch to
- If `Ok`
    - Set the thread state to running
    - Update CPU_CURRENT_THREAD_ARRAY
    - Run the thread
- If `Err`
    - This means that a different' CPU "stole" this thread
    - Call RUN_THREAD_FN

### DESTROY_THREAD_IPI handler
- If you are running a thread
    - Check if the thread you are running needs to be destroyed
    - If it does:
        - Call DESTROY_THREAD_FN
    - Else
        - Call CLEAN_UP_THREADS_FN
- Else
    - Call CLEAN_UP_THREADS_FN

### CLEAN_UP_THREADS_FN
- Go through every thread in THREAD_VEC and find the first thread that needs to be destroyed. If there is none, that's `unreachable!`
- Try to acquire the thread's mutex
- If `Ok`
    - Call DESTROY_THREAD_FN
- If `Err(thread_id)`
    - Send a DESTROY_THREAD_IPI to `thread_id`

### DESTROY_THREAD_FN
- TODO: Clean up resources maybe
- Acquire a lock to the threads
- Remove the item from the threads

### Handling an interrupt event happening
- If the interrupt needs to cause side-effects, such as writing to an event stream, then do that.
- Get the event in the EVENTS_MAP
- If the event is `Pending`
    - Do nothing, go back to the thread you were running before if you were running one.
- If the event is `NotPending`
    - If `Some(thread_id)`
        - Push the event onto the events array. TODO: how to do this
        - Mark the thread as ready to run
        - Call ASSIGN_THREAD
    - If `None`
        - Make the event `Pending`

### WAIT_UNTIL_EVENTS syscall handler
- Loop through the slice of events to wait for
    - Get the event in the EVENTS_MAP
    - If `Pending`
        - Change it to `NotPending`
        - Push the event onto the return slice. TODO: how to do this
    - If `NotPending(None)`
        - Change it to `NotPending(Some(thread_id))`
    - If `NotPending(Some(thread_id))`
        - Return an error
- If any events were already pending:
    - Return from the syscall
- Else
    - Set the thread state to waiting
    - Call RUN_THREAD_FN

# Events
Whenever a thread is not actively using the CPU (or ready to use the CPU), it is waiting for an event. The EVENTS_MAP is used to efficiently handle interrupts and other events and wake up threads that are waiting for that event to happen. When a thread uses the wait until events syscall, it claims the events that it's waiting for. This makes the OS very `async` Rust friendly. 

## Possible Bugs
### 2 CPUs spawn a thread at the same time
In this case, both CPUs would send a RUN_THREAD IPI to the same CPU (let's assume it was sleeping). Because of the way the compare and exchange is used, it will be ensured that the higher priority threads will always be run by the CPUs.