use core::num::NonZero;

use alloc::vec::Vec;
use common::{
    ElfSegmentFlags, EnvEntry, LOWER_HALF_END, MapModule, MemProt, PageSize, STACK_ALIGNMENT,
    SpawnProcessMemoryFlags, SpawnProcessMemoryMapping, SpawnProcessRelativePriority,
};
use elf::{ElfBytes, endian::NativeEndian};
use user_lib::{
    ENV_KEY, KEYBOARD_ENV_KEY, RustSyscallSpawnProcessInput, syscall_alloc, syscall_map_module,
    syscall_spawn_process,
};
use x86_64::{
    VirtAddr,
    structures::paging::{Page, Size4KiB},
};

pub fn spawn_process(
    module_id: usize,
    priority: SpawnProcessRelativePriority,
    window_shared_mem_capability: NonZero<u64>,
    keyboard_shared_mem_capability: NonZero<u64>,
    send_capabilities: &[NonZero<u64>],
) -> NonZero<u32> {
    let slice = syscall_map_module(module_id).unwrap();
    let elf = ElfBytes::<NativeEndian>::minimal_parse(slice).unwrap();
    let entry_point = elf.ehdr.e_entry;
    let mut memory_mappings = Vec::<SpawnProcessMemoryMapping>::new();
    let mut map_modules = Vec::<MapModule>::new();
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
        let flags = ElfSegmentFlags::from_bits_retain(segment.p_flags);
        if flags.contains(ElfSegmentFlags::WRITABLE) {
            let start_page = Page::<Size4KiB>::containing_address(VirtAddr::new(segment.p_vaddr));
            let end_page = Page::<Size4KiB>::containing_address(VirtAddr::new(
                segment.p_vaddr + (segment.p_memsz - 1),
            ));
            for page in start_page..=end_page {
                let mut frame = syscall_alloc(
                    PageSize::_4KiB,
                    1.try_into().unwrap(),
                    // TODO: No need to zero the entire page
                    true,
                    MemProt::READABLE | MemProt::WRITABLE,
                )
                .unwrap();
                memory_mappings.push(SpawnProcessMemoryMapping {
                    current_process_start: frame.addr().into(),
                    new_process_start: page.start_address().as_u64() as usize,
                    pages_len: 1,
                    flags: SpawnProcessMemoryFlags::from(flags).bits(),
                });
                let frame_data = unsafe { frame.as_mut() };
                let bytes_to_zero_before = segment
                    .p_vaddr
                    .saturating_sub(page.start_address().as_u64())
                    .min(PageSize::_4KiB.byte_len_u64());
                let range_before_to_zero = ..bytes_to_zero_before as usize;
                frame_data[range_before_to_zero].fill(0);

                let copy_start = bytes_to_zero_before;
                let already_copied = page
                    .start_address()
                    .as_u64()
                    .saturating_sub(segment.p_vaddr)
                    .min(segment.p_filesz);
                let copy_end = (copy_start + (segment.p_filesz - already_copied))
                    .min(PageSize::_4KiB.byte_len_u64());
                let copy_len = copy_end - copy_start;
                let range_to_copy = copy_start as usize..copy_end as usize;
                frame_data[range_to_copy].copy_from_slice(
                    &segment_data[already_copied as usize..(already_copied + copy_len) as usize],
                );

                let range_after_to_zero = copy_end as usize..;
                frame_data[range_after_to_zero].fill(0);
            }
        } else {
            map_modules.push(MapModule {
                module_id,
                start_page_offset: segment.p_offset as usize / PageSize::_4KiB.byte_len(),
                pages_len: ((segment.p_offset as usize + segment.p_filesz as usize)
                    .div_ceil(PageSize::_4KiB.byte_len())
                    - (segment.p_offset as usize) / PageSize::_4KiB.byte_len())
                .try_into()
                .unwrap(),
                new_process_start: segment.p_vaddr as usize / PageSize::_4KiB.byte_len()
                    * PageSize::_4KiB.byte_len(),
                executable: flags.contains(ElfSegmentFlags::EXECUTABLE),
            });
        }
    }
    let stack_top = LOWER_HALF_END as usize;
    let stack_len = 64 * 0x400;
    let stack_with_guard_len = PageSize::_4KiB.byte_len() + stack_len;
    let stack = syscall_alloc(
        PageSize::_4KiB,
        (1 + stack_len / PageSize::_4KiB.byte_len())
            .try_into()
            .unwrap(),
        true,
        MemProt::READABLE | MemProt::WRITABLE,
    )
    .unwrap();

    let env_entries = &[
        EnvEntry {
            key: ENV_KEY,
            value: window_shared_mem_capability.get(),
        },
        EnvEntry {
            key: KEYBOARD_ENV_KEY,
            value: keyboard_shared_mem_capability.get(),
        },
    ];
    let env_size = size_of_val(env_entries) + size_of::<u64>();
    // Make sure the pointer is aligned by 16
    let env_ptr = (usize::from(stack.addr()) + stack.len() - env_size) / STACK_ALIGNMENT as usize
        * STACK_ALIGNMENT as usize;

    let entries_count_ptr = env_ptr as *mut u64;
    let entries_count = env_entries.len() as u64;
    unsafe { entries_count_ptr.write(entries_count) };
    let entries_ptr = (env_ptr + size_of::<u64>()) as *mut EnvEntry;
    let entries_len = env_entries.len();
    let entries = unsafe { core::slice::from_raw_parts_mut(entries_ptr, entries_len) };
    entries.copy_from_slice(env_entries);

    // Guard page
    memory_mappings.push(SpawnProcessMemoryMapping {
        current_process_start: stack.addr().into(),
        new_process_start: stack_top - stack_with_guard_len,
        pages_len: 1,
        flags: SpawnProcessMemoryFlags::empty().bits(),
    });
    // Stack
    memory_mappings.push(SpawnProcessMemoryMapping {
        current_process_start: stack.addr().get() + PageSize::_4KiB.byte_len(),
        new_process_start: stack_top - stack_len,
        pages_len: stack_len / PageSize::_4KiB.byte_len(),
        flags: (SpawnProcessMemoryFlags::READABLE | SpawnProcessMemoryFlags::WRITABLE).bits(),
    });

    // common::log::info!("Memory mappings: {memory_mappings:#X?}. Map modules: {map_modules:#X?}");

    syscall_spawn_process(RustSyscallSpawnProcessInput {
        priority,
        rip: entry_point,
        rsp: u64::try_from(stack_top).unwrap()
            - u64::try_from(usize::from(stack.addr()) + stack.len() - env_ptr).unwrap(),
        send_memory: &memory_mappings,
        send_capabilities,
        map_modules: &map_modules,
    })
    .unwrap()
}
