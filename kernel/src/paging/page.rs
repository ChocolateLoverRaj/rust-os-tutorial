use common::PageSize;
use x86_64::VirtAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page {
    start_addr: VirtAddr,
    size: PageSize,
}

#[derive(Debug)]
pub enum NewPageError {
    NotAligned,
}

impl Page {
    pub fn new(start_addr: VirtAddr, size: PageSize) -> Result<Self, NewPageError> {
        if start_addr.is_aligned(size.byte_len_u64()) {
            Ok(Self { start_addr, size })
        } else {
            Err(NewPageError::NotAligned)
        }
    }

    pub fn start_addr(&self) -> VirtAddr {
        self.start_addr
    }

    pub fn size(&self) -> PageSize {
        self.size
    }

    pub fn offset(&self, page_count: u64) -> Option<Self> {
        let bytes_offset = page_count.checked_mul(self.size.byte_len_u64())?;
        let offset_start_addr = self.start_addr.as_u64().checked_add(bytes_offset)?;
        Some(Self::new(VirtAddr::new(offset_start_addr), self.size).unwrap())
    }
}
