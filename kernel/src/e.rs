use core::{num::NonZero, slice};

use elf::{ElfBytes, endian::NativeEndian};
use limine::response::ModuleResponse;

use crate::{
    stack_sizes_iterator::StackSizesIterator, user_mode_program_path::USER_MODE_PROGRAM_PATH,
};

pub fn e(module_response: &ModuleResponse) {
    if let Some(file) = module_response
        .modules()
        .iter()
        .find(|file| file.path() == USER_MODE_PROGRAM_PATH)
    {
        // Safety: Limine gaves us a valid pointer and len
        let file = unsafe { slice::from_raw_parts(file.addr(), file.size() as usize) };
        let elf = ElfBytes::<NativeEndian>::minimal_parse(file).unwrap();
        let entry_point = NonZero::try_from(elf.ehdr.e_entry).unwrap();

        let entry_point_stack_size = StackSizesIterator::try_parse_from_elf(&elf)
            .unwrap()
            .unwrap()
            .map(Result::unwrap)
            .find(|item| item.symbol == entry_point.into())
            .unwrap()
            .stack_size;
        log::debug!("Entry point stack size: 0x{entry_point_stack_size:X}");
    } else {
        log::warn!("No module found");
    }
}
