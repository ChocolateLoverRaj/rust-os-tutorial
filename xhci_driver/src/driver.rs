use core::{
    fmt::Debug,
    num::NonZero,
    ptr::{NonNull, slice_from_raw_parts_mut},
};

use alloc::{boxed::Box, vec};
use volatile::VolatilePtr;

use crate::{
    CapabilityRegs, CapabilityRegsVolatileFieldAccess, OperationalRegs,
    OperationalRegsVolatileFieldAccess,
};

#[derive(Debug)]
pub struct Driver {
    cap_regs: VolatilePtr<'static, CapabilityRegs>,
    /// The xHCI DCBAA is an array of physical addresses.
    /// This driver also stores an array of virtual addresses.
    dcbaa: Option<Box<[Option<NonZero<usize>>]>>,
}

impl Driver {
    /// # Safety
    /// You must find the address and size of BAR 0, and then create a mapping for the entire BAR 0 with the correct memory type (caching behavior).
    /// Then you input the virtual address that points to the start of BAR 0. The BAR should be an xHCI device's BAR.
    pub unsafe fn new(bar0: NonZero<usize>) -> Self {
        Self {
            cap_regs: {
                let pointer =
                    NonNull::new(bar0.get() as *mut CapabilityRegs).expect("ptr is not null");
                // Safety: the caller promised that the pointer points to xHCI memory
                unsafe { VolatilePtr::new(pointer) }
            },
            dcbaa: None,
        }
    }

    fn op_regs(&mut self) -> VolatilePtr<OperationalRegs> {
        let pointer = NonNull::new(
            (self.cap_regs.as_raw_ptr().addr().get() + self.cap_regs.cap_length().read() as usize)
                as *mut OperationalRegs,
        )
        .expect("ptr is not null");
        unsafe { VolatilePtr::new(pointer) }
    }

    pub fn debug_capability_registers(&mut self) -> impl Debug {
        self.cap_regs.read()
    }

    pub fn debug_operational_registers(&mut self) -> impl Debug {
        self.op_regs().read()
    }

    pub fn reset_host_controller(&mut self) {
        let op_regs = self.op_regs();
        // Reset the host controller
        op_regs.usb_cmd().update(|mut cmd| {
            cmd.set_run(false);
            cmd
        });
        // TODO: Timeout after 200ms
        while !op_regs.usb_sts().read().hch() {}

        op_regs.usb_cmd().update(|mut cmd| {
            cmd.set_host_controller_reset(true);
            cmd
        });
        // TODO: Timeout after 1000ms
        while op_regs.usb_cmd().read().host_controller_reset() || op_regs.usb_sts().read().cnr() {}

        // TODO: On real hardware, wait for 50ms - https://youtu.be/9rI_fYvng6Q?list=PLATP7rOKo3E82tBnMp90B4zejpWeAKlxn&t=359
        if op_regs.usb_cmd().read().0 != 0 {
            panic!()
        }
        if op_regs.dn_ctrl().read().0 != 0 {
            panic!()
        }
        if op_regs.crcr().read().0 != 0 {
            panic!()
        }
        if op_regs.dcbaap().read().0 != 0 {
            panic!()
        }
        if op_regs.config().read().0 != 0 {
            panic!()
        }
    }

    pub fn configure_operational_registers(&mut self) {
        // Enable device notifications
        self.op_regs().dn_ctrl().update(|mut dn_ctrl| {
            dn_ctrl.set_notification_enable(u16::MAX);
            dn_ctrl
        });

        let max_slots = self.cap_regs.hcs_params_1().read().max_slots();
        self.op_regs().config().update(|mut config| {
            config.set_max_slots_en(max_slots);
            config
        });
    }

    /// This function is useful for calling [`Self::set_up_dcbaa`].
    /// If `None` is returned, you do not need scratchpad pages.
    pub fn required_scratchpad_pages(&mut self) -> Option<NonZero<u8>> {
        NonZero::new(self.cap_regs.hcs_params_2().read().max_scratchpad_buffers())
    }

    /// # Safety
    /// The pages must be valid and not used for anything else
    pub unsafe fn set_up_dcbaa(&mut self, input: SetUpDcbaaInput) {
        // We need 1 slot for the scratchpad array, and 1 slot for each possible device
        let dcbaa_len = 1 + self.cap_regs.hcs_params_1().read().max_slots() as usize;
        let mut dcbaa_ptr = NonNull::new(slice_from_raw_parts_mut(
            input.dcbaa_page.virt_addr.get() as *mut u64,
            dcbaa_len,
        ))
        .expect("ptr is not null");
        // Safety: The caller promised that the virt addr points to the page
        let dcbaa = unsafe { dcbaa_ptr.as_mut() };
        // Initialize the dcbaa with 0s
        dcbaa.fill(0);
        let mut driver_virt_dcbaa = vec![None; dcbaa_len].into_boxed_slice();

        if let Some(required_scratchpad_pages) = self.required_scratchpad_pages() {
            let scratchpad_pages = input
                .scratchpad_pages
                .expect("This xHCI needs scratchpad pages");
            assert_eq!(
                required_scratchpad_pages.get() as usize,
                scratchpad_pages.scratchpad_pages.len(),
                "the correct number of scratchpad pages should be allocated"
            );
            let mut scratchpad_array_ptr = NonNull::new(slice_from_raw_parts_mut(
                scratchpad_pages.scratchpad_array_page.virt_addr.get() as *mut u64,
                scratchpad_pages.scratchpad_pages.len(),
            ))
            .expect("ptr is not null");
            // Safety: the scratchpad array is 1 page, and the scratchpad array fits in 1 page
            let scratchpad_array = unsafe { scratchpad_array_ptr.as_mut() };
            for (i, scratchpad_page) in scratchpad_pages.scratchpad_pages.iter().enumerate() {
                scratchpad_array[i] = scratchpad_page.phys_addr;
            }

            // The first dcbaa item is a pointer to the scratchpads array
            dcbaa[0] = scratchpad_pages.scratchpad_array_page.phys_addr;
            driver_virt_dcbaa[0] = Some(scratchpad_pages.scratchpad_array_page.virt_addr);
        }

        self.op_regs().dcbaap().update(|mut dcbaap| {
            dcbaap.set_dcbaap(input.dcbaa_page.phys_addr);
            dcbaap
        });

        self.dcbaa = Some(driver_virt_dcbaa)
    }
}

/// A physical frame that is also mapped in virtual memory as uncacheable.
#[derive(Debug)]
pub struct XhciPage {
    pub phys_addr: u64,
    pub virt_addr: NonZero<usize>,
}

#[derive(Debug)]
pub struct ScratchpadPages {
    // TODO: Since the scratchpad array could be <1 page, we could save memory by sharing a page with other xHCI memory.
    /// A page used for the array of pointers to scratchpad pages
    pub scratchpad_array_page: XhciPage,
    /// The scratchpad pages themselves
    pub scratchpad_pages: Box<[XhciPage]>,
}

#[derive(Debug)]
pub struct SetUpDcbaaInput {
    pub dcbaa_page: XhciPage,
    pub scratchpad_pages: Option<ScratchpadPages>,
}
