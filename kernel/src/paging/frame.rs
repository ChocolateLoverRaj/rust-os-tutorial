use common::PageSize;
use x86_64::PhysAddr;

#[derive(Debug, Clone, Copy)]
pub struct Frame {
    start_addr: PhysAddr,
    size: PageSize,
}

#[derive(Debug)]
pub enum NewFrameError {
    NotAligned,
}

impl Frame {
    pub fn new(start_addr: PhysAddr, size: PageSize) -> Result<Self, NewFrameError> {
        if start_addr.is_aligned(size.byte_len_u64()) {
            Ok(Self { start_addr, size })
        } else {
            Err(NewFrameError::NotAligned)
        }
    }

    pub fn start_addr(&self) -> PhysAddr {
        self.start_addr
    }

    pub fn size(&self) -> PageSize {
        self.size
    }

    pub fn offset(&self, page_count: u64) -> Option<Self> {
        let bytes_offset = page_count.checked_mul(self.size.byte_len_u64())?;
        let offset_start_addr = self.start_addr.as_u64().checked_add(bytes_offset)?;
        Some(Self::new(PhysAddr::new(offset_start_addr), self.size).unwrap())
    }
}
