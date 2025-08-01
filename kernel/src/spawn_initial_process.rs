use core::{num::NonZero, slice};

use alloc::{collections::btree_set::BTreeSet, sync::Arc};
use common::{
    ENV_PS2_KEYBOARD_CAPABILITY, ElfSegmentFlags, EnvEntry, LOWER_HALF_END, STACK_ALIGNMENT,
};
use elf::{ElfBytes, endian::NativeEndian};
use limine::{file::File, response::ModuleResponse};
use nodit::{Interval, NoditMap, OverlapError};
use spin::RwLock;
use thiserror::Error;
use x86_64::{
    PhysAddr, VirtAddr,
    addr::VirtAddrNotValid,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageSize, PageTableFlags, PhysFrame, Size4KiB,
        mapper::MapToError,
    },
};

use crate::{
    capabilities::{CAPABILITIES, Capability, CapabilityId, CapabilityType},
    get_page_table::get_page_table,
    hhdm_offset::HhdmOffset,
    memory::{MEMORY, MemoryType},
    smep_smap::{clac, has_smap, stac},
    task::{
        Process, ProcessId, ProcessMemory, StartData, THREAD_PRIORITIES, THREADS, Thread, ThreadId,
        ThreadReadyState, ThreadState, UserVirtMem,
    },
    translate_addr::{GetFrameSlice, ZeroFrame},
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
    MapToError(MapToError<Size4KiB>),
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
        let user_l4_frame = FrameAllocator::<Size4KiB>::allocate_frame(
            &mut physical_memory.get_user_mode_program_frame_allocator(process_id),
        )
        .ok_or(LoadUserModeProgramError::OutOfMemory)?;
        // Safety: frame is offset mapped and it's a new table
        let mut mapper = unsafe { get_page_table(user_l4_frame, true) };

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
                        (segment.p_vaddr / Size4KiB::SIZE * Size4KiB::SIZE
                            ..(segment.p_vaddr + segment.p_memsz).next_multiple_of(Size4KiB::SIZE))
                            .into(),
                        UserVirtMem::LimineModule,
                    )
                    .map_err(LoadUserModeProgramError::OverlappingElfSegments)?;
                let start_page = Page::<Size4KiB>::containing_address(
                    VirtAddr::try_new(segment.p_vaddr)
                        .map_err(LoadUserModeProgramError::InvalidVirtAddr)?,
                );
                let page_count = (segment.p_vaddr + segment.p_memsz).div_ceil(Size4KiB::SIZE)
                    - segment.p_vaddr / Size4KiB::SIZE;
                let first_frame = PhysFrame::<Size4KiB>::from_start_address(PhysAddr::new(
                    file.addr() as u64 - u64::from(HhdmOffset::get_from_response()),
                ))
                .unwrap()
                    + segment.p_offset / Size4KiB::SIZE;
                log::debug!("Saving {page_count} frames by zerocopy ELF");
                for i in 0..page_count {
                    let page = start_page + i;
                    let frame = first_frame + i;
                    let flags = PageTableFlags::PRESENT
                        | PageTableFlags::USER_ACCESSIBLE
                        | flags.to_page_table_flags();
                    let mut frame_allocator =
                        physical_memory.get_user_mode_program_frame_allocator(process_id);
                    unsafe { mapper.map_to(page, frame, flags, &mut frame_allocator) }
                        .unwrap()
                        .ignore();
                }
                let mapped_last_frame = (segment.p_offset + segment.p_filesz) / Size4KiB::SIZE
                    == elf_bytes.len() as u64 / Size4KiB::SIZE;
                if mapped_last_frame {
                    todo!("Need to make sure unused bytes in the frame are zeroed")
                }
            } else {
                let segment_data = elf
                    .segment_data(&segment)
                    .map_err(LoadUserModeProgramError::ElfParseError)?;
                let start_page = Page::<Size4KiB>::containing_address(
                    VirtAddr::try_new(segment.p_vaddr)
                        .map_err(LoadUserModeProgramError::InvalidVirtAddr)?,
                );
                let end_page = Page::<Size4KiB>::containing_address(
                    VirtAddr::try_new({
                        let end_addr_inclusive =
                            segment
                                .p_vaddr
                                .checked_add(segment.p_memsz - 1)
                                .ok_or(LoadUserModeProgramError::OutOfBoundsMemory)?;
                        if end_addr_inclusive >= LOWER_HALF_END {
                            Err(LoadUserModeProgramError::OutOfBoundsMemory)?;
                        }
                        end_addr_inclusive
                    })
                    .map_err(LoadUserModeProgramError::InvalidVirtAddr)?,
                );
                log::debug!(
                    "Using {} frames for writable segment",
                    end_page - start_page + 1
                );
                mapped_virtual_memory
                    .insert_merge_touching_if_values_equal(
                        (start_page.start_address().as_u64()
                            ..=(end_page.start_address() + (end_page.size() - 1)).as_u64())
                            .into(),
                        UserVirtMem::Plain,
                    )
                    .map_err(LoadUserModeProgramError::OverlappingElfSegments)?;
                for page in start_page..=end_page {
                    let frame = physical_memory
                        .allocate_frame_with_type(MemoryType::UsedByUserMode(BTreeSet::from([
                            process_id,
                        ])))
                        .ok_or(LoadUserModeProgramError::OutOfMemory)?;
                    let flags = PageTableFlags::PRESENT
                        | PageTableFlags::USER_ACCESSIBLE
                        | flags.to_page_table_flags();
                    // log::info!("Mapping {page:?}->{frame:?} with flags: {flags:?}");
                    unsafe {
                        mapper.map_to(
                            page,
                            frame,
                            flags,
                            &mut physical_memory.get_user_mode_program_frame_allocator(process_id),
                        )
                    }
                    .map_err(LoadUserModeProgramError::MapToError)?
                    // The Cr3 has not been loaded with this page table yet
                    .ignore();
                    let frame_data = unsafe { frame.get_slice_mut() };
                    let bytes_to_zero_before = segment
                        .p_vaddr
                        .saturating_sub(page.start_address().as_u64())
                        .min(Size4KiB::SIZE);
                    let range_before_to_zero = ..bytes_to_zero_before as usize;
                    frame_data[range_before_to_zero].fill(0);

                    let copy_start = bytes_to_zero_before;
                    let already_copied = page
                        .start_address()
                        .as_u64()
                        .saturating_sub(segment.p_vaddr)
                        .min(segment.p_filesz);
                    let copy_end =
                        (copy_start + (segment.p_filesz - already_copied)).min(Size4KiB::SIZE);
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
        let stack_end_inclusive = INITIAL_RSP - 1;
        let stack_start = INITIAL_RSP - stack_size;
        mapped_virtual_memory
            .insert_merge_touching(
                (stack_start..=stack_end_inclusive).into(),
                UserVirtMem::Plain,
            )
            .map_err(LoadUserModeProgramError::OverlappingElfSegmentsAndStack)?;
        let stack_start_page =
            Page::<Size4KiB>::from_start_address(VirtAddr::new(stack_start)).unwrap();
        let stack_end_page_inclusive =
            Page::<Size4KiB>::containing_address(VirtAddr::new(stack_end_inclusive));
        for page in stack_start_page..=stack_end_page_inclusive {
            let frame = physical_memory
                .allocate_frame_with_type(MemoryType::UsedByUserMode(BTreeSet::from([process_id])))
                .ok_or(LoadUserModeProgramError::OutOfMemory)?;
            // Safety: We just claimed this frame
            unsafe { frame.zero() };
            let flags = PageTableFlags::PRESENT
                | PageTableFlags::USER_ACCESSIBLE
                | PageTableFlags::WRITABLE
                | PageTableFlags::NO_EXECUTE;
            unsafe {
                mapper.map_to(
                    page,
                    frame,
                    flags,
                    &mut physical_memory.get_user_mode_program_frame_allocator(process_id),
                )
            }
            .unwrap()
            .ignore();
        }

        // Safety: phys mem is valid and offset mapped
        let current_l4_page_table = unsafe { get_page_table(memory.new_kernel_cr3, false) };
        // Copy the kernel's page tables
        let level_4_table_mut = mapper.level_4_table_mut();
        let current_level_4_table = current_l4_page_table.level_4_table();
        for i in 256..512 {
            level_4_table_mut[i].clone_from(&current_level_4_table[i]);
        }
        unsafe { Cr3::write(user_l4_frame, memory.new_kernel_cr3_flags) };

        let process = Arc::new(Process {
            id: process_id,
            cr3: user_l4_frame,
            memory: RwLock::new(ProcessMemory {
                mapped_virtual_memory,
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

        log::info!("New process's Cr3: {user_l4_frame:?}");
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
