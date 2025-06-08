use core::{
    alloc::Layout,
    mem::MaybeUninit,
    slice,
    sync::atomic::{AtomicBool, Ordering},
};

use common::SyscallMemInfoOutput;
use spin::Lazy;
use talc::{OomHandler, Talc, Talck};
use x86_64::{
    VirtAddr,
    structures::paging::{Page, PageSize, Size4KiB},
};

use crate::syscalls::{syscall_alloc, syscall_alloc_2, syscall_mem_info};

static POSITION: spin::Mutex<u64> = spin::Mutex::new(Size4KiB::SIZE);
static MEM_INFO: Lazy<SyscallMemInfoOutput> = Lazy::new(syscall_mem_info);
pub static FAILED_TO_ALLOCATE: AtomicBool = AtomicBool::new(false);

struct MyOomHandler;

impl OomHandler for MyOomHandler {
    fn handle_oom(talc: &mut talc::Talc<Self>, layout: core::alloc::Layout) -> Result<(), ()> {
        let result = (|| {
            assert!(layout.align() as u64 <= Size4KiB::SIZE);
            let mut position = POSITION.try_lock().unwrap();
            let pages_needed = (layout.size() as u64).div_ceil(Size4KiB::SIZE) + 1;
            let mut used_mem = [&MEM_INFO.elf, &MEM_INFO.stack, &(800000000000..=u64::MAX)];
            used_mem.sort_by_key(|range| range.start());
            let mut used_mem_iter = used_mem.iter();
            let position = loop {
                if let Some(range) = used_mem_iter.next() {
                    let end_barrier_exclusive = range.start();
                    if let Some(bytes_before) = end_barrier_exclusive.checked_sub(*position) {
                        let full_pages_before = bytes_before / Size4KiB::SIZE;
                        // skipped_pages += skipped_pages_now;
                        // *position += skipped_pages_now * Size4KiB::SIZE;
                        if full_pages_before >= pages_needed {
                            *position += pages_needed * Size4KiB::SIZE;
                            break Some(position);
                        } else {
                            *position = range.end() + 1;
                        }
                    }
                } else {
                    break None;
                }
            }
            .ok_or(())?;
            unsafe {
                syscall_alloc(
                    Page::from_start_address(VirtAddr::new(*position)).unwrap(),
                    pages_needed,
                )
            }
            .map_err(|_| ())?;
            let allocated_memory = unsafe {
                slice::from_raw_parts_mut(
                    *position as *mut MaybeUninit<u8>,
                    (pages_needed * Size4KiB::SIZE) as usize,
                )
            };
            let span = allocated_memory.into();
            unsafe { talc.claim(span) }?;
            Ok(())
        })();
        if result.is_err() {
            FAILED_TO_ALLOCATE.store(true, Ordering::Release);
        }
        result
    }
}

struct MyOomHandler2;

impl OomHandler for MyOomHandler2 {
    fn handle_oom(talc: &mut talc::Talc<Self>, layout: core::alloc::Layout) -> Result<(), ()> {
        assert!(layout.align() as u64 <= Size4KiB::SIZE);
        let result = (|| {
            let bytes_needed = {
                let is_first_heap = talc.get_counters().heap_count == 0;
                let overhead_len = if is_first_heap {
                    // talc says "~1 KiB", so we'll assume 1.5 KiB to be safe
                    0x600
                } else {
                    size_of::<usize>()
                }
                .next_multiple_of(layout.align());
                layout.size() + overhead_len
            };
            let slice =
                syscall_alloc_2(Layout::from_size_align(bytes_needed, layout.align()).unwrap())
                    .map_err(|_| ())?;
            let span = slice.into();
            unsafe { talc.claim(span) }?;
            Ok(())
        })();
        if result.is_err() {
            FAILED_TO_ALLOCATE.store(true, Ordering::Release);
        }
        result
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: Talck<spin::Mutex<()>, MyOomHandler2> =
    Talck::new(Talc::new(MyOomHandler2));
