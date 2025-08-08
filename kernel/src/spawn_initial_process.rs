use core::{num::NonZero, slice};

use alloc::sync::Arc;
use common::{
    ENV_PS2_KEYBOARD_CAPABILITY, ElfSegmentFlags, EnvEntry, LOWER_HALF_END, PageSize,
    STACK_ALIGNMENT,
};
use elf::{ElfBytes, endian::NativeEndian};
use limine::{file::File, response::ModuleResponse};
use nodit::{Interval, NoditMap, OverlapError};
use spin::RwLock;
use thiserror::Error;
use x86_64::{
    VirtAddr,
    addr::VirtAddrNotValid,
    structures::paging::{FrameAllocator, Size4KiB},
};

use crate::{
    EffectiveFlags, Frame, MapPageError2, Page,
    capabilities::{CAPABILITIES, Capability, CapabilityId, CapabilityType},
    memory::{MEMORY, MemoryType},
    smep_smap::{clac, has_smap, stac},
    task::{
        Process, ProcessId, ProcessMemory, StartData, THREAD_PRIORITIES, THREADS, Thread, ThreadId,
        ThreadReadyState, ThreadState, UserVirtMem,
    },
    translate_addr::{TranslateToPhys, TranslateToVirt, ZeroFrame},
    user_mode_program_path::USER_MODE_PROGRAM_PATH,
};

/// If this was a normal pointer and not the stack pointer, this address would be invalid because it is not canonical.
/// However, since this is a stack pointer it is still technically pointing to the lower half so this actually works.
pub const INITIAL_RSP: u64 = LOWER_HALF_END;

#[derive(Debug, Error)]
enum LoadUserModeProgramError {
    #[error("Error parsing the ELF")]
    ElfParseError(elf::ParseError),
    #[error("The ELF has no entry point specified")]
    NoEntryPoint,
    #[error("Could not allocate physical memory")]
    OutOfMemory,
    #[error("No segment table")]
    NoSegmentTable,
    #[error("ELF has overlapping loadable segments")]
    OverlappingElfSegments(OverlapError<UserVirtMem>),
    #[error("Error creating a page table mapping")]
    MapPageError(MapPageError2),
    #[error("ELF tried to use higher half virtual memory")]
    OutOfBoundsMemory,
    #[error("The ELF specified an invalid virtual address")]
    InvalidVirtAddr(VirtAddrNotValid),
    #[error("ELF segments overlap with the stack")]
    OverlappingElfSegmentsAndStack(OverlapError<UserVirtMem>),
}

fn spawn_task(file: &File) -> Result<(), LoadUserModeProgramError> {
    // Safety: Limine gives us a valid pointer and len
    let elf_bytes = unsafe { slice::from_raw_parts(file.addr(), file.size() as usize) };
    let memory = MEMORY.get().unwrap();
    let mut physical_memory = memory.physical_memory.lock();
    let process_id = ProcessId::new_unique();
    let mut try_spawn = || {
        let elf = ElfBytes::<NativeEndian>::minimal_parse(elf_bytes)
            .map_err(LoadUserModeProgramError::ElfParseError)?;
        // It's okay if the entry point is invalid, we will handle the page fault gracefully
        let entry_point =
            NonZero::new(elf.ehdr.e_entry).ok_or(LoadUserModeProgramError::NoEntryPoint)?;
        let mut process_l4 = {
            let frame = FrameAllocator::<Size4KiB>::allocate_frame(
                &mut physical_memory.get_user_mode_program_frame_allocator(process_id),
            )
            .ok_or(LoadUserModeProgramError::OutOfMemory)?;
            let mut virt_mem = memory.virtual_memory.lock();
            unsafe { virt_mem.new_user_page_table(frame) }
        };

        let mut mapped_virtual_memory = NoditMap::<u64, Interval<_>, UserVirtMem>::default();

        let segments = elf
            .segments()
            .ok_or(LoadUserModeProgramError::NoSegmentTable)?;
        for segment in segments
            .iter()
            .filter(|segment| segment.p_type == 1)
            .filter(|segment| segment.p_memsz > 0)
        {
            // log::debug!("Segment: {segment:#X?}");
            let flags = ElfSegmentFlags::from(segment);

            // The ELF should be in a way so that read-only segments are zerocopy
            // We shouldn't need to copy data into new phys frames
            // We should be able to just map directly to Limine's phys frames
            // TODO: Properly validate this to handle untrusted ELFs
            if !flags.contains(ElfSegmentFlags::WRITABLE) {
                assert_eq!(segment.p_memsz, segment.p_filesz);
                mapped_virtual_memory
                    .insert_merge_touching_if_values_equal(
                        (segment.p_vaddr / PageSize::_4KiB.byte_len_u64()
                            * PageSize::_4KiB.byte_len_u64()
                            ..(segment.p_vaddr + segment.p_memsz)
                                .next_multiple_of(PageSize::_4KiB.byte_len_u64()))
                            .into(),
                        UserVirtMem::LimineModule,
                    )
                    .map_err(LoadUserModeProgramError::OverlappingElfSegments)?;
                let start_page = Page::new(
                    VirtAddr::try_new(segment.p_vaddr)
                        .map_err(LoadUserModeProgramError::InvalidVirtAddr)?
                        .align_down(PageSize::_4KiB.byte_len_u64()),
                    PageSize::_4KiB,
                )
                .unwrap();
                let page_count = (segment.p_vaddr + segment.p_memsz)
                    .div_ceil(PageSize::_4KiB.byte_len_u64())
                    - segment.p_vaddr / PageSize::_4KiB.byte_len_u64();
                let first_frame = Frame::new(
                    (VirtAddr::from_ptr(file.addr()) + segment.p_offset)
                        .align_down(PageSize::_4KiB.byte_len_u64())
                        .to_phys_offset_mapped(),
                    PageSize::_4KiB,
                )
                .unwrap();
                log::debug!("Saving {page_count} frames by zerocopy ELF");
                for i in 0..page_count {
                    let page = start_page.offset(i).unwrap();
                    let frame = first_frame.offset(i).unwrap();
                    let flags = EffectiveFlags {
                        writable: false,
                        executable: flags.contains(ElfSegmentFlags::EXECUTABLE),
                        global: false,
                        user_accessible: true,
                    };
                    let mut frame_allocator =
                        physical_memory.get_user_mode_program_frame_allocator(process_id);
                    unsafe { process_l4.map_page(page, frame, flags, &mut frame_allocator) }
                        .unwrap();
                }
                let mapped_last_frame = (segment.p_offset + segment.p_filesz)
                    / PageSize::_4KiB.byte_len_u64()
                    == elf_bytes.len() as u64 / PageSize::_4KiB.byte_len_u64();
                if mapped_last_frame {
                    todo!("Need to make sure unused bytes in the frame are zeroed")
                }
            } else {
                let segment_data = elf
                    .segment_data(&segment)
                    .map_err(LoadUserModeProgramError::ElfParseError)?;
                let start_page = Page::new(
                    VirtAddr::try_new(segment.p_vaddr)
                        .map_err(LoadUserModeProgramError::InvalidVirtAddr)?
                        .align_down(PageSize::_4KiB.byte_len_u64()),
                    PageSize::_4KiB,
                )
                .unwrap();
                let end_addr = segment
                    .p_vaddr
                    .checked_add(segment.p_memsz)
                    .ok_or(LoadUserModeProgramError::OutOfBoundsMemory)?;
                if end_addr > LOWER_HALF_END {
                    Err(LoadUserModeProgramError::OutOfBoundsMemory)?;
                }
                let pages_len = end_addr.div_ceil(PageSize::_4KiB.byte_len_u64())
                    - segment.p_vaddr / PageSize::_4KiB.byte_len_u64();

                mapped_virtual_memory
                    .insert_merge_touching_if_values_equal(
                        (segment.p_vaddr / PageSize::_4KiB.byte_len_u64()
                            * PageSize::_4KiB.byte_len_u64()
                            ..end_addr.next_multiple_of(PageSize::_4KiB.byte_len_u64()))
                            .into(),
                        UserVirtMem::Plain,
                    )
                    .map_err(LoadUserModeProgramError::OverlappingElfSegments)?;
                for i in 0..pages_len {
                    let page = start_page.offset(i).unwrap();
                    let frame = physical_memory
                        .allocate_frame_with_type(
                            PageSize::_4KiB,
                            MemoryType::UsedByUserMode(process_id),
                        )
                        .ok_or(LoadUserModeProgramError::OutOfMemory)?;
                    let flags = EffectiveFlags {
                        writable: flags.contains(ElfSegmentFlags::WRITABLE),
                        executable: flags.contains(ElfSegmentFlags::EXECUTABLE),
                        global: false,
                        user_accessible: true,
                    };
                    log::trace!("Mapping {page:?}->{frame:?} with flags: {flags:?}");
                    let mut frame_allocator =
                        physical_memory.get_user_mode_program_frame_allocator(process_id);
                    unsafe { process_l4.map_page(page, frame, flags, &mut frame_allocator) }
                        .map_err(LoadUserModeProgramError::MapPageError)?;
                    let frame_data = {
                        let ptr = frame.start_addr().to_virt().as_mut_ptr::<u8>();
                        let len = frame.size().byte_len();
                        unsafe { slice::from_raw_parts_mut(ptr, len) }
                    };
                    let bytes_to_zero_before = segment
                        .p_vaddr
                        .saturating_sub(page.start_addr().as_u64())
                        .min(PageSize::_4KiB.byte_len_u64());
                    let range_before_to_zero = ..bytes_to_zero_before as usize;
                    frame_data[range_before_to_zero].fill(0);

                    let copy_start = bytes_to_zero_before;
                    let already_copied = page
                        .start_addr()
                        .as_u64()
                        .saturating_sub(segment.p_vaddr)
                        .min(segment.p_filesz);
                    let copy_end = (copy_start + (segment.p_filesz - already_copied))
                        .min(PageSize::_4KiB.byte_len_u64());
                    let copy_len = copy_end - copy_start;
                    let range_to_copy = copy_start as usize..copy_end as usize;
                    // log::debug!("Copying {range_to_copy:X?}");
                    frame_data[range_to_copy].copy_from_slice(
                        &segment_data
                            [already_copied as usize..(already_copied + copy_len) as usize],
                    );

                    let range_after_to_zero = copy_end as usize..;
                    // log::debug!("Zeroing (after): {range_after_to_zero:X?}");
                    frame_data[range_after_to_zero].fill(0);
                }
            }
        }
        // Map the stack
        let stack_size = 64 * 0x400;
        let stack_start = INITIAL_RSP - stack_size;
        // Guard page
        mapped_virtual_memory
            .insert_merge_touching((stack_start..INITIAL_RSP).into(), UserVirtMem::Plain)
            .map_err(LoadUserModeProgramError::OverlappingElfSegmentsAndStack)?;
        let stack_start_page = Page::new(VirtAddr::new(stack_start), PageSize::_4KiB).unwrap();
        let pages_len = stack_size / PageSize::_4KiB.byte_len_u64();
        for i in 0..pages_len {
            let page = stack_start_page.offset(i).unwrap();
            let frame = physical_memory
                .allocate_frame_with_type(PageSize::_4KiB, MemoryType::UsedByUserMode(process_id))
                .ok_or(LoadUserModeProgramError::OutOfMemory)?;
            // Safety: We just claimed this frame
            unsafe { frame.zero() };
            let flags = EffectiveFlags {
                writable: true,
                executable: false,
                user_accessible: true,
                global: false,
            };
            let mut frame_allocator =
                physical_memory.get_user_mode_program_frame_allocator(process_id);
            unsafe { process_l4.map_page(page, frame, flags, &mut frame_allocator) }.unwrap();
        }

        log::debug!("Mapped virt mem: {mapped_virtual_memory:#X?}");
        unsafe { process_l4.switch_to(memory.new_kernel_cr3_flags) };

        let process = Arc::new(Process {
            id: process_id,
            cr3: process_l4.frame(),
            memory: RwLock::new(ProcessMemory {
                mapped_virtual_memory,
                l4: process_l4,
            }),
            mutexes: Default::default(),
        });

        let env_entries = &[{
            let id = CapabilityId::new_unique();
            CAPABILITIES.write().insert(
                id.into(),
                Capability {
                    _type: CapabilityType::Ps2Keyboard,
                    process_id: process.id.into(),
                },
            );
            EnvEntry {
                key: ENV_PS2_KEYBOARD_CAPABILITY,
                value: NonZero::from(id).get(),
            }
        }];
        let env_size = size_of_val(env_entries) + size_of::<u64>();
        // Make sure the pointer is aligned by 16
        let env_ptr = (stack_start as usize + stack_size as usize - env_size)
            / STACK_ALIGNMENT as usize
            * STACK_ALIGNMENT as usize;

        if has_smap() {
            stac();
        }
        let entries_count_ptr = env_ptr as *mut u64;
        let entries_count = env_entries.len() as u64;
        unsafe { entries_count_ptr.write(entries_count) };
        let entries_ptr = (env_ptr + size_of::<u64>()) as *mut EnvEntry;
        let entries_len = env_entries.len();
        let entries = unsafe { core::slice::from_raw_parts_mut(entries_ptr, entries_len) };
        entries.copy_from_slice(env_entries);
        if has_smap() {
            clac();
        }

        let thread_id = ThreadId::new_unique();
        THREADS.write().insert(
            thread_id,
            Thread {
                state: RwLock::new(ThreadState::Ready(ThreadReadyState::ReadyToStart(
                    StartData {
                        rip: entry_point.into(),
                        rsp: env_ptr as u64,
                    },
                ))),
                process,
            },
        );
        THREAD_PRIORITIES.write().push(thread_id);
        Ok(())
    };
    match try_spawn() {
        Ok(input) => Ok(input),
        Err(error) => {
            // Before we return the error, we must clean up any memory used by the user space program
            physical_memory.remove_user_mode_memory();
            // Because it errored, the Cr3 was not switched so we don't need to worry about switching it back
            Err(error)
        }
    }
}

/// Creates a process with a single thread, based on an ELF
pub fn spawn_initial_process(module_response: &ModuleResponse) {
    if let Some(file) = module_response
        .modules()
        .iter()
        .find(|file| file.path() == USER_MODE_PROGRAM_PATH)
    {
        match spawn_task(file) {
            Ok(_) => {
                log::debug!("Spawned task");
            }
            Err(e) => {
                log::warn!("Error loading ELF: {e:#?}");
            }
        };
    } else {
        log::warn!("No module found");
    }
}
