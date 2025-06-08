use core::{num::NonZero, ops::RangeInclusive, slice};

use common::SyscallMemInfoOutput;
use elf::{ElfBytes, endian::NativeEndian};
use limine::response::ModuleResponse;
use nodit::{Interval, NoditSet, OverlapError};
use thiserror::Error;
use x86_64::{
    VirtAddr,
    addr::VirtAddrNotValid,
    registers::{control::Cr3, rflags::RFlags},
    structures::paging::{
        FrameAllocator, Mapper, Page, PageSize, PageTableFlags, PhysFrame, Size4KiB,
        mapper::MapToError,
    },
};

use crate::{
    elf_flags_to_page_table_flags::elf_flags_to_page_table_flags,
    enter_user_mode::{EnterUserModeInput, enter_user_mode},
    get_page_table::get_page_table,
    hlt_loop::hlt_loop,
    memory::{MEMORY, MemoryType, UserModeMemoryUsageType},
    translate_addr::GetFrameSlice,
    user_mode_program_path::USER_MODE_PROGRAM_PATH,
};

pub struct Task {
    pub cr3: PhysFrame,
    pub mapped_virtual_memory: NoditSet<u64, Interval<u64>>,
    pub mem_info: SyscallMemInfoOutput,
}

pub static TASK: spin::Mutex<Option<Task>> = spin::Mutex::new(None);

// If this was a normal pointer and not the stack pointer, this address would be invalid because it is not canonical.
// However, since this is a stack pointer it is still technically pointing to the lower half so this actually works.
pub const INITIAL_RSP: u64 = 0x800000000000;

pub fn run_user_mode_program(module_response: &ModuleResponse) -> ! {
    if let Some(file) = module_response
        .modules()
        .iter()
        .find(|file| file.path() == USER_MODE_PROGRAM_PATH)
    {
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
            OverlappingElfSegments(OverlapError<()>),
            #[error("Error creating a page table mapping")]
            MapToError(MapToError<Size4KiB>),
            #[error("ELF tried to use higher half virtual memory")]
            OutOfBoundsMemory,
            #[error("The ELF specified an invalid virtual address")]
            InvalidVirtAddr(VirtAddrNotValid),
            #[error("ELF segments overlap with the stack")]
            OverlappingElfSegmentsAndStack(OverlapError<()>),
            #[error("The ELF has 0 segments")]
            NoSegments,
        }
        fn run_user_mode_program(
            elf: &[u8],
        ) -> Result<EnterUserModeInput, LoadUserModeProgramError> {
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            match (|| {
                Ok({
                    let elf = ElfBytes::<NativeEndian>::minimal_parse(elf)
                        .map_err(LoadUserModeProgramError::ElfParseError)?;
                    // It's okay if the entry point is invalid, we will handle the page fault gracefully
                    let entry_point = NonZero::new(elf.ehdr.e_entry)
                        .ok_or(LoadUserModeProgramError::NoEntryPoint)?;
                    if u64::from(entry_point) >= 0x800000000000 {
                        Err(LoadUserModeProgramError::OutOfBoundsMemory)?;
                    }
                    let user_l4_frame = FrameAllocator::<Size4KiB>::allocate_frame(
                        &mut physical_memory.get_user_mode_program_frame_allocator(),
                    )
                    .ok_or(LoadUserModeProgramError::OutOfMemory)?;
                    // Safety: frame is offset mapped and it's a new table
                    let mut mapper = unsafe { get_page_table(user_l4_frame, true) };

                    let mut mapped_virtual_memory = NoditSet::<u64, Interval<_>>::default();
                    let mut elf_range = None::<RangeInclusive<u64>>;
                    let segments = elf
                        .segments()
                        .ok_or(LoadUserModeProgramError::NoSegmentTable)?;
                    if segments.is_empty() {
                        Err(LoadUserModeProgramError::NoSegments)?;
                    }
                    for segment in segments
                        .iter()
                        .filter(|segment| segment.p_type == 1)
                        .filter(|segment| segment.p_memsz > 0)
                    {
                        log::debug!("Segment: {segment:#X?}");
                        let segment_data = elf
                            .segment_data(&segment)
                            .map_err(LoadUserModeProgramError::ElfParseError)?;
                        let start_page = Page::<Size4KiB>::containing_address(
                            VirtAddr::try_new(segment.p_vaddr)
                                .map_err(LoadUserModeProgramError::InvalidVirtAddr)?,
                        );
                        let end_page = Page::<Size4KiB>::containing_address(
                            VirtAddr::try_new({
                                let end_addr_inclusive = segment
                                    .p_vaddr
                                    .checked_add(segment.p_memsz - 1)
                                    .ok_or(LoadUserModeProgramError::OutOfBoundsMemory)?;
                                if end_addr_inclusive >= 0x800000000000 {
                                    Err(LoadUserModeProgramError::OutOfBoundsMemory)?;
                                }
                                end_addr_inclusive
                            })
                            .map_err(LoadUserModeProgramError::InvalidVirtAddr)?,
                        );
                        let mem_range = start_page.start_address().as_u64()
                            ..=(end_page.start_address() + (end_page.size() - 1)).as_u64();
                        match elf_range.as_mut() {
                            Some(elf_range) => {
                                *elf_range = *elf_range.start().min(mem_range.start())
                                    ..=*elf_range.end().max(mem_range.end());
                            }
                            None => elf_range = Some(mem_range.clone()),
                        }
                        mapped_virtual_memory
                            .insert_merge_touching(mem_range.into())
                            .map_err(LoadUserModeProgramError::OverlappingElfSegments)?;
                        for page in start_page..=end_page {
                            let frame = physical_memory
                                .allocate_frame_with_type(MemoryType::UsedByUserMode(
                                    UserModeMemoryUsageType::Elf,
                                ))
                                .ok_or(LoadUserModeProgramError::OutOfMemory)?;
                            let flags = PageTableFlags::PRESENT
                                | PageTableFlags::USER_ACCESSIBLE
                                | elf_flags_to_page_table_flags(segment.p_flags);
                            log::info!("Mapping {page:?}->{frame:?} with flags: {flags:?}");
                            unsafe {
                                mapper.map_to(
                                    page,
                                    frame,
                                    flags,
                                    &mut physical_memory.get_user_mode_program_frame_allocator(),
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
                            log::debug!("Zeroeing (before) {range_before_to_zero:X?}");
                            frame_data[range_before_to_zero].fill(0);

                            let copy_start = bytes_to_zero_before;
                            let already_copied = page
                                .start_address()
                                .as_u64()
                                .saturating_sub(segment.p_vaddr)
                                .min(segment.p_filesz);
                            let copy_end = (copy_start + (segment.p_filesz - already_copied))
                                .min(Size4KiB::SIZE);
                            let copy_len = copy_end - copy_start;
                            let range_to_copy = copy_start as usize..copy_end as usize;
                            log::debug!("Copying {range_to_copy:X?}");
                            frame_data[range_to_copy].copy_from_slice(
                                &segment_data
                                    [already_copied as usize..(already_copied + copy_len) as usize],
                            );

                            let range_after_to_zero = copy_end as usize..;
                            log::debug!("Zeroing (after): {range_after_to_zero:X?}");
                            frame_data[range_after_to_zero].fill(0);
                        }
                    }
                    // Map the stack
                    let stack_size = 64 * 0x400;
                    let stack_end_inclusive = INITIAL_RSP - 1;
                    let stack_start = INITIAL_RSP - stack_size;
                    mapped_virtual_memory
                        .insert_merge_touching((stack_start..=stack_end_inclusive).into())
                        .map_err(LoadUserModeProgramError::OverlappingElfSegmentsAndStack)?;
                    let stack_start_page =
                        Page::<Size4KiB>::from_start_address(VirtAddr::new(stack_start)).unwrap();
                    let stack_end_page_inclusive =
                        Page::<Size4KiB>::containing_address(VirtAddr::new(stack_end_inclusive));
                    for page in stack_start_page..=stack_end_page_inclusive {
                        let frame = physical_memory
                            .allocate_frame_with_type(MemoryType::UsedByUserMode(
                                UserModeMemoryUsageType::Stack,
                            ))
                            .ok_or(LoadUserModeProgramError::OutOfMemory)?;
                        let flags = PageTableFlags::PRESENT
                            | PageTableFlags::USER_ACCESSIBLE
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::NO_EXECUTE;
                        unsafe {
                            mapper.map_to(
                                page,
                                frame,
                                flags,
                                &mut physical_memory.get_user_mode_program_frame_allocator(),
                            )
                        }
                        .unwrap()
                        .ignore();
                    }

                    // Safety: phys mem is valid and offset mapped
                    let current_l4_page_table =
                        unsafe { get_page_table(memory.new_kernel_cr3, false) };
                    // Copy the kernel's page tables
                    let level_4_table_mut = mapper.level_4_table_mut();
                    let current_level_4_table = current_l4_page_table.level_4_table();
                    for i in 256..512 {
                        level_4_table_mut[i].clone_from(&current_level_4_table[i]);
                    }
                    unsafe { Cr3::write(user_l4_frame, memory.new_kernel_cr3_flags) };
                    *TASK.lock() = Some(Task {
                        cr3: user_l4_frame,
                        mapped_virtual_memory,
                        mem_info: SyscallMemInfoOutput {
                            elf: {
                                // We alrady made sure there was at least 1 segment so this should never panic
                                elf_range.unwrap()
                            },
                            stack: stack_start..=stack_end_inclusive,
                        },
                    });
                    EnterUserModeInput {
                        rip: VirtAddr::new(entry_point.into()),
                        rsp: INITIAL_RSP,
                        rflags: RFlags::INTERRUPT_FLAG,
                    }
                })
            })() {
                Ok(input) => Ok(input),
                Err(error) => {
                    // Before we return the error, we must clean up any memory used by the user space program
                    physical_memory.remove_user_mode_memory();
                    // Because it errored, the Cr3 was not switched so we don't need to worry about switching it back
                    Err(error)
                }
            }
        }
        // Safety: Limine gaves us a valid pointer and len
        let file = unsafe { slice::from_raw_parts(file.addr(), file.size() as usize) };
        match run_user_mode_program(file) {
            Ok(input) => {
                log::debug!("Entering user mode");
                unsafe { enter_user_mode(input) };
            }
            Err(e) => {
                log::warn!("Error loading ELF: {e:#?}");
                hlt_loop()
            }
        };
    } else {
        log::warn!("No module found");
        hlt_loop()
    }
}
