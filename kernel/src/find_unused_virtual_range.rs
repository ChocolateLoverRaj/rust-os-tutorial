use core::ops::Range;

use x86_64::structures::paging::{Page, Size4KiB};

use crate::page_tables_traverser::PageTablesTraverser;

pub fn find_unused_virtual_range(
    mut page_tables_traverser: PageTablesTraverser,
    n_4kib_pages: u64,
) -> Option<Range<Page<Size4KiB>>> {
    let mut range = None;
    loop {
        let entry = page_tables_traverser.next()?;
        if entry.present {
            range = None;
        } else {
            let start_page =
                Page::<Size4KiB>::from_start_address(entry.page_table_index_stack.start_address())
                    .unwrap();
            let range = range.get_or_insert(start_page..start_page);
            range.end = start_page + entry.page_table_index_stack.page_len();
            if range.end - range.start >= n_4kib_pages {
                break;
            }
        }
    }
    range
}
