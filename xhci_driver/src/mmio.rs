use core::num::NonZero;

#[derive(Debug)]
pub struct XhciMmio {
    /// The virtual address to the mapped xHCI memory
    pub(crate) addr: NonZero<usize>,
}

impl XhciMmio {
    /// # Safety
    /// You must find the address and size of BAR 0, and then create a mapping for the entire BAR 0 with the correct memory type (caching behavior).
    /// Then you input the virtual address that points to the start of BAR 0. The BAR should be an xHCI device's BAR.
    pub unsafe fn new(addr: NonZero<usize>) -> Self {
        Self { addr }
    }
}
