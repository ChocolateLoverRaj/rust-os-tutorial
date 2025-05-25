use limine::response::HhdmResponse;
use x86_64::{
    VirtAddr,
    structures::paging::{PageTable, PageTableFlags, PhysFrame, Size4KiB},
};

#[derive(Debug, Clone)]
pub struct PageTableVirtualPage {
    indexes: heapless::Vec<usize, 4>,
}

impl PageTableVirtualPage {
    pub fn start_address(&self) -> VirtAddr {
        let mut page_start_addr = 0;
        let mut shift_by = 12 + 9 + 9 + 9;
        for index in &self.indexes {
            page_start_addr += index << shift_by;
            shift_by -= 9;
        }
        VirtAddr::new_truncate(page_start_addr as u64)
    }

    /// Get the length of the page table (or missing entry) as a multiple of 4KiB
    pub fn n_4kib_pages(&self) -> u64 {
        512_u64.pow((4 - self.indexes.len()).try_into().unwrap())
    }

    /// Get the length of the page table (or missing entry)
    pub fn page_len(&self) -> u64 {
        0x1000 * self.n_4kib_pages()
    }
}

#[derive(Debug, Clone)]
pub struct PageTableEntry {
    pub page_table_index_stack: PageTableVirtualPage,
    /// If the present bit is set in the flags
    pub present: bool,
}

/// Recursively traverses page table, returning every mapping / unused slot
pub struct PageTablesTraverser {
    hhdm: &'static HhdmResponse,
    top_level_page_table: PhysFrame<Size4KiB>,
    parent_page_tables_entry_index_stack: heapless::Vec<usize, 3>,
    entry_index: usize,
}

impl PageTablesTraverser {
    /// The `initial_entry_index` is initial entry index in the top level page table to start at.
    /// For example, use 256 to start in the higher half of the virtual address space.
    ///
    /// # Safety
    /// The top level page table and its entries must be actually mapped
    pub unsafe fn new(
        hhdm: &'static HhdmResponse,
        top_level_page_table: PhysFrame<Size4KiB>,
        initial_entry_index: usize,
    ) -> Self {
        Self {
            hhdm,
            top_level_page_table,
            parent_page_tables_entry_index_stack: Default::default(),
            entry_index: initial_entry_index,
        }
    }
}

impl Iterator for PageTablesTraverser {
    type Item = PageTableEntry;

    fn next(&mut self) -> Option<Self::Item> {
        let active_l4_pt = {
            let active_l4_pt = (self.top_level_page_table.start_address().as_u64()
                + self.hhdm.offset()) as *const PageTable;
            unsafe { &*active_l4_pt }
        };

        loop {
            // We went through every entry in the table
            if self.entry_index == 512 {
                if let Some(parent_entry_index) = self.parent_page_tables_entry_index_stack.pop() {
                    // We finished going through a L3, L2, or L1 page table, go to the next entry in the parent table
                    self.entry_index = parent_entry_index + 1;
                    continue;
                } else {
                    // We finished going through the L4 page table so we're done
                    break None;
                }
            }
            // Get the current page table which has the entry that we're going to process
            let pt = {
                // Start with the L4 pt
                let mut pt = active_l4_pt;
                // Traverse the page tables until we get to the lowest level we want to process
                for index in &self.parent_page_tables_entry_index_stack {
                    let pt_ptr =
                        (pt[*index].addr().as_u64() + self.hhdm.offset()) as *const PageTable;
                    pt = unsafe { &*pt_ptr };
                }
                pt
            };
            let entry = &pt[self.entry_index];
            let get_page_table_index_stack = || PageTableVirtualPage {
                indexes: {
                    let mut indexes = heapless::Vec::<_, 4>::from_slice(
                        &self.parent_page_tables_entry_index_stack,
                    )
                    .unwrap();
                    indexes.push(self.entry_index).unwrap();
                    indexes
                },
            };
            if entry.flags().contains(PageTableFlags::PRESENT) {
                if entry.flags().contains(PageTableFlags::HUGE_PAGE)
                    || self.parent_page_tables_entry_index_stack.len() == 3
                {
                    // This entry point to a phys frame, which could be 4KiB, 2MiB, or 1GiB
                    // Note that just cuz PageTableFlags::HUGE_PAGE is 1 doesn't mean that it's >4KiB - see https://github.com/phil-opp/blog_os/issues/1403
                    let page_table_entry = PageTableEntry {
                        page_table_index_stack: get_page_table_index_stack(),
                        present: true,
                    };
                    self.entry_index += 1;
                    break Some(page_table_entry);
                } else {
                    // This entry points to another entry
                    self.parent_page_tables_entry_index_stack
                        .push(self.entry_index)
                        .unwrap();
                    self.entry_index = 0;
                    continue;
                }
            } else {
                let page_table_entry = PageTableEntry {
                    page_table_index_stack: get_page_table_index_stack(),
                    present: false,
                };
                self.entry_index += 1;
                break Some(page_table_entry);
            }
        }
    }
}
