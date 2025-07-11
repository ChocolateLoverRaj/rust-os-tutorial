use alloc::vec::Vec;
use common::{
    AllocPageSize, ElfSegmentFlags, EnvEntry, SpawnProcessMemoryFlags, SpawnProcessMemoryMapping,
    SpawnProcessRelativePriority,
};
use elf::{ElfBytes, endian::NativeEndian};
use user_lib::{
    ENV_KEY, RustSyscallSpawnProcessInput, WindowSharedMemServer, syscall_alloc,
    syscall_map_module, syscall_spawn_process,
};
use x86_64::{
    VirtAddr,
    structures::paging::{Page, PageSize, Size4KiB},
};

pub fn spawn_process(
    module_id: usize,
    priority: SpawnProcessRelativePriority,
    // env_entries: &[EnvEntry],
    // send_rx_list: impl Iterator<Item = Sender>,
    window: &WindowSharedMemServer,
) {
    let slice = syscall_map_module(module_id).unwrap();
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
                segment
                    .p_memsz
                    .next_multiple_of(AllocPageSize::_4KiB.size_bytes())
                    .try_into()
                    .unwrap(),
                AllocPageSize::_4KiB,
            )
            .unwrap();
            memory_mappings.push(SpawnProcessMemoryMapping {
                current_process_start: frame.addr() as u64,
                new_process_start: page.start_address().as_u64(),
                len: page.size(),
                flags: SpawnProcessMemoryFlags::from(ElfSegmentFlags::from_bits_retain(
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
    let stack_with_guard_len = Size4KiB::SIZE + stack_len;
    let stack = syscall_alloc(
        stack_with_guard_len.try_into().unwrap(),
        AllocPageSize::_4KiB,
    )
    .unwrap();

    // FIXME: make sure this doesn't conflict with other mem
    let new_process_shared_mem_ptr = 0x40000000;

    let env_entries = &[EnvEntry {
        key: ENV_KEY,
        value: new_process_shared_mem_ptr,
    }];
    let env_size = size_of_val(env_entries) + size_of::<u64>();
    // Make sure the pointer is aligned by 16
    let env_ptr = (stack.addr() + stack.len() - env_size) / 16 * 16;

    let entries_count_ptr = env_ptr as *mut u64;
    let entries_count = env_entries.len() as u64;
    unsafe { entries_count_ptr.write(entries_count) };
    let entries_ptr = (env_ptr + size_of::<u64>()) as *mut EnvEntry;
    let entries_len = env_entries.len();
    let entries = unsafe { core::slice::from_raw_parts_mut(entries_ptr, entries_len) };
    entries.copy_from_slice(env_entries);

    // Guard page
    memory_mappings.push(SpawnProcessMemoryMapping {
        current_process_start: stack.addr() as u64,
        new_process_start: stack_top - stack_with_guard_len as u64,
        len: 0x1000,
        flags: SpawnProcessMemoryFlags::empty().bits(),
    });
    memory_mappings.push(SpawnProcessMemoryMapping {
        current_process_start: stack.addr() as u64 + 0x1000,
        new_process_start: stack_top - stack_len as u64,
        len: stack_len as u64,
        flags: (SpawnProcessMemoryFlags::READABLE | SpawnProcessMemoryFlags::WRITABLE).bits(),
    });

    // let shared_page = syscall_alloc(Layout::from_size_align(0x1000, 0x1000).unwrap()).unwrap();
    memory_mappings.push(SpawnProcessMemoryMapping {
        current_process_start: window.addr().try_into().unwrap(),
        new_process_start: new_process_shared_mem_ptr,
        len: window.size().try_into().unwrap(),
        flags: (SpawnProcessMemoryFlags::READABLE
            | SpawnProcessMemoryFlags::WRITABLE
            | SpawnProcessMemoryFlags::SHARE
            | SpawnProcessMemoryFlags::_2MiB_PAGE)
            .bits(),
    });

    syscall_spawn_process(RustSyscallSpawnProcessInput {
        priority,
        rip: entry_point,
        rsp: stack_top - (stack.addr() + stack.len() - env_ptr) as u64,
        memory_mapping: &memory_mappings,
        send_channels: &[window.channel_id()],
    });
}
