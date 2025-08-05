use core::alloc::Layout;

use common::{MemProt, PageSize, SyscallAllocError};
use spin::Once;
use talc::{OomHandler, Talc, Talck};

use crate::{mutex::RawBlockingLock, syscalls::syscall_alloc};

#[derive(Debug)]
pub struct AllocError {
    pub layout: Layout,
    pub error: SyscallAllocError,
}

pub static ALLOC_ERROR: Once<AllocError> = Once::new();

struct MyOomHandler;

impl OomHandler for MyOomHandler {
    fn handle_oom(talc: &mut talc::Talc<Self>, layout: core::alloc::Layout) -> Result<(), ()> {
        assert!(layout.align() <= PageSize::_4KiB.byte_len());
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
            let slice = syscall_alloc(
                PageSize::_4KiB,
                bytes_needed
                    .div_ceil(PageSize::_4KiB.byte_len())
                    .try_into()
                    .unwrap(),
                // Talck will zero it anyways, so we don't need the kernel to also zero it
                false,
                MemProt::READABLE | MemProt::WRITABLE,
            )?;
            let span = slice.as_ptr().into();
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
static GLOBAL_ALLOCATOR: Talck<RawBlockingLock, MyOomHandler> = Talck::new(Talc::new(MyOomHandler));
