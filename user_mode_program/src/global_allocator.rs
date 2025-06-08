use core::alloc::Layout;

use common::SyscallAllocError;
use spin::Once;
use talc::{OomHandler, Talc, Talck};
use x86_64::structures::paging::{PageSize, Size4KiB};

use crate::syscalls::syscall_alloc;

#[derive(Debug)]
pub struct AllocError {
    pub layout: Layout,
    pub error: SyscallAllocError,
}

pub static ALLOC_ERROR: Once<AllocError> = Once::new();

struct MyOomHandler;

impl OomHandler for MyOomHandler {
    fn handle_oom(talc: &mut talc::Talc<Self>, layout: core::alloc::Layout) -> Result<(), ()> {
        assert!(layout.align() as u64 <= Size4KiB::SIZE);
        let result = (|| {
            let bytes_needed = {
                let is_first_heap = talc.get_counters().heap_count == 0;
                let overhead_len = if is_first_heap {
                    // talc says "~1 KiB", so we'll assume 1.5 KiB to be safe
                    0x600
                } else {
                    // Based on the talc `claim` method
                    size_of::<usize>()
                }
                .next_multiple_of(layout.align());
                layout.size() + overhead_len
            };
            let slice =
                syscall_alloc(Layout::from_size_align(bytes_needed, layout.align()).unwrap())?;
            let span = slice.into();
            unsafe { talc.claim(span) }.unwrap();
            Ok::<_, SyscallAllocError>(())
        })();
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                ALLOC_ERROR.call_once(|| AllocError { layout, error });
                Err(())
            }
        }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: Talck<spin::Mutex<()>, MyOomHandler> = Talck::new(Talc::new(MyOomHandler));
