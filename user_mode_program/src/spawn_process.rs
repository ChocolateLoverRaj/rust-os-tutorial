use core::alloc::Layout;

use alloc::vec::Vec;
use common::{
    ElfSegmentFlags, PagePermissions, ProcessRelativePriority, SpawnProcessMemoryMapping, log,
};
use elf::{ElfBytes, endian::NativeEndian};
use user_lib::{
    RustSyscallSpawnProcessInput, syscall_alloc, syscall_map_module, syscall_spawn_process,
};
use x86_64::{
    VirtAddr,
    structures::paging::{Page, PageSize, Size4KiB},
};

pub fn spawn_process() {
    let slice = syscall_map_module(0).unwrap();
    let elf = ElfBytes::<NativeEndian>::minimal_parse(slice).unwrap();
    let entry_point = elf.ehdr.e_entry;
    let mut memory_mappings = Vec::<SpawnProcessMemoryMapping>::new();
    for segment in elf
        .segments()
        .unwrap()
        .iter()
        // Only map loadable segments
        .filter(|segment| segment.p_type == 1)
        // Skip empty segments
        .filter(|segment| segment.p_memsz > 0)
    {
        let segment_data = elf.segment_data(&segment).unwrap();
        let start_page = Page::<Size4KiB>::containing_address(VirtAddr::new(segment.p_vaddr));
        let end_page = Page::<Size4KiB>::containing_address(VirtAddr::new(
            segment.p_vaddr + (segment.p_memsz - 1),
        ));
        for page in start_page..=end_page {
            let frame = syscall_alloc(
                Layout::from_size_align(segment.p_memsz as usize, segment.p_align as usize)
                    .unwrap(),
            )
            .unwrap();
            memory_mappings.push(SpawnProcessMemoryMapping {
                current_process_start: frame.addr() as u64,
                new_process_start: page.start_address().as_u64(),
                len: page.size(),
                permissions: PagePermissions::from(ElfSegmentFlags::from_bits_retain(
                    segment.p_flags,
                ))
                .bits(),
            });
            let frame_data = unsafe { frame.as_mut().unwrap() };
            let bytes_to_zero_before = segment
                .p_vaddr
                .saturating_sub(page.start_address().as_u64())
                .min(Size4KiB::SIZE);
            let range_before_to_zero = ..bytes_to_zero_before as usize;
            // log::debug!("Zeroeing (before) {range_before_to_zero:X?}");
            frame_data[range_before_to_zero].fill(0);

            let copy_start = bytes_to_zero_before;
            let already_copied = page
                .start_address()
                .as_u64()
                .saturating_sub(segment.p_vaddr)
                .min(segment.p_filesz);
            let copy_end = (copy_start + (segment.p_filesz - already_copied)).min(Size4KiB::SIZE);
            let copy_len = copy_end - copy_start;
            let range_to_copy = copy_start as usize..copy_end as usize;
            frame_data[range_to_copy].copy_from_slice(
                &segment_data[already_copied as usize..(already_copied + copy_len) as usize],
            );

            let range_after_to_zero = copy_end as usize..;
            frame_data[range_after_to_zero].fill(0);
        }
    }
    let stack_top = 0x800000000000;
    let stack_len = 64 * 0x400;
    let stack_with_guard_len = 0x1000 + stack_len;
    let stack =
        syscall_alloc(Layout::from_size_align(stack_with_guard_len, 0x1000).unwrap()).unwrap();
    log::debug!("Stack: {stack:p}");
    // Guard page
    memory_mappings.push(SpawnProcessMemoryMapping {
        current_process_start: stack.addr() as u64,
        new_process_start: stack_top - stack_with_guard_len as u64,
        len: 0x1000,
        permissions: PagePermissions::empty().bits(),
    });
    memory_mappings.push(SpawnProcessMemoryMapping {
        current_process_start: stack.addr() as u64 + 0x1000,
        new_process_start: stack_top - stack_len as u64,
        len: stack_len as u64,
        permissions: (PagePermissions::READABLE | PagePermissions::WRITABLE).bits(),
    });

    syscall_spawn_process(RustSyscallSpawnProcessInput {
        priority: ProcessRelativePriority::Higher,
        rip: entry_point,
        rsp: stack_top,
        memory_mapping: &memory_mappings,
    });
}
