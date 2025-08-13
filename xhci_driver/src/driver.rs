use core::{
    fmt::Debug,
    num::NonZero,
    ptr::{NonNull, slice_from_raw_parts_mut},
};

use alloc::{boxed::Box, vec, vec::Vec};
use volatile::VolatilePtr;

use crate::*;

type Dcbaa = Box<[Option<NonZero<usize>>]>;

#[derive(Debug)]
pub struct DriverOp {
    /// The xHCI DCBAA is an array of physical addresses.
    /// This driver also stores an array of virtual addresses.
    dcbaa: Dcbaa,
    command_ring: XhciCommandRing,
}

// These are configurable. These values are what the tutorial uses.
const XHCI_COMMAND_RING_TRB_COUNT: NonZero<usize> = NonZero::new(256).unwrap();
const XHCI_EVENT_RING_TRB_COUNT: NonZero<u32> = NonZero::new(256).unwrap();

#[derive(Debug)]
pub struct Driver {
    cap_regs: VolatilePtr<'static, CapabilityRegs>,
    /// The xHCI DCBAA is an array of physical addresses.
    /// This driver also stores an array of virtual addresses.
    op: Option<DriverOp>,
    event_ring: Option<XhciEventRing>,
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
            op: None,
            event_ring: None,
        }
    }

    pub fn init_req(&mut self) -> Box<[Option<MultiAllocRequest>]> {
        let mut requests = Vec::new();
        requests.extend_from_slice(&self.configure_operational_registers_req());
        requests.extend_from_slice(&self.configure_runtime_registers_req());
        requests.into_boxed_slice()
    }

    /// Before calling this function, call [`Self::init_req`] so you can allocate memory.
    ///
    /// # Safety
    /// The pages must be valid and not used for anything else
    pub unsafe fn init(&mut self, res: &[MultiAllocResponse]) {
        assert_responses_match(&self.init_req(), &res);
        self.reset_host_controller();
        self.configure_operational_registers((&res[0..4]).try_into().unwrap());
        self.configure_runtime_registers((&res[4..6]).try_into().unwrap());
    }

    fn op_regs(&mut self) -> VolatilePtr<OperationalRegs> {
        let pointer = NonNull::new(
            (self.cap_regs.as_raw_ptr().addr().get() + self.cap_regs.cap_length().read() as usize)
                as *mut OperationalRegs,
        )
        .expect("ptr is not null");
        unsafe { VolatilePtr::new(pointer) }
    }

    fn runtime_regs(&mut self) -> VolatilePtr<RuntimeRegisters> {
        let pointer = NonNull::new(
            (self.cap_regs.as_raw_ptr().addr().get() + self.cap_regs.rts_off().read() as usize)
                as *mut RuntimeRegisters,
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

    fn configure_operational_registers_req(&mut self) -> [Option<MultiAllocRequest>; 4] {
        let scratchpad_buffers = NonZero::new(self.required_scratchpad_buffers() as u64);
        [
            // For the DCBAA
            Some(MultiAllocRequest {
                request: AllocRequest {
                    size: NonZero::new((self.required_dcbaa_len() * size_of::<u64>()) as u64)
                        .expect("DCBAA len > 0"),
                    align: XHCI_DEVICE_CONTEXT_ALIGNMENT,
                    boundary: XHCI_DEVICE_CONTEXT_BOUNDARY,
                },
                count: NonZero::new(1).unwrap(),
            }),
            // For the scratchpads array itself
            scratchpad_buffers.map(|scratchpad_buffers| MultiAllocRequest {
                request: AllocRequest {
                    size: NonZero::new(scratchpad_buffers.get() * size_of::<u64>() as u64)
                        .expect("at least 1 scratchpad buffer"),
                    align: XHCI_DEVICE_CONTEXT_ALIGNMENT,
                    boundary: XHCI_DEVICE_CONTEXT_BOUNDARY,
                },
                count: NonZero::new(1).unwrap(),
            }),
            // For the scratchpad buffers
            scratchpad_buffers.map(|scratchpad_buffers| MultiAllocRequest {
                request: AllocRequest {
                    size: PAGE_SIZE,
                    align: XHCI_SCRATCHPAD_BUFFERS_ALIGNMENT,
                    boundary: XHCI_SCRATCHPAD_BUFFERS_BOUNDARY,
                },
                count: NonZero::new(scratchpad_buffers.get() as usize)
                    .expect("at least 1 scratchpad buffer"),
            }),
            // For the command ring
            Some(MultiAllocRequest {
                request: XhciCommandRing::new_req(XHCI_COMMAND_RING_TRB_COUNT),
                count: NonZero::new(1).unwrap(),
            }),
        ]
    }

    /// Before calling this function, call [`Self::configure_operational_registers_req`] so you can allocate memory.
    ///
    /// # Safety
    /// The pages must be valid and not used for anything else
    fn configure_operational_registers(&mut self, res: &[MultiAllocResponse; 4]) {
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

        let dcbaa = self.set_up_dcbaa([res[0].clone(), res[1].clone(), res[2].clone()]);

        let command_ring = XhciCommandRing::new(XHCI_COMMAND_RING_TRB_COUNT, res[3][0]);
        self.op_regs().crcr().write({
            let mut crcr = Crcr(0);
            crcr.set_command_ring_ptr(command_ring.phys_addr());
            crcr.set_ring_cycle_state(command_ring.cycle_bit());
            crcr
        });

        self.op = Some(DriverOp {
            dcbaa,
            command_ring,
        });
    }

    fn required_scratchpad_buffers(&mut self) -> u8 {
        self.cap_regs.hcs_params_2().read().max_scratchpad_buffers()
    }

    /// Returns the number of `u64`s needed for the DCBAA
    fn required_dcbaa_len(&mut self) -> usize {
        // We need 1 slot for the scratchpad array, and 1 slot for each possible device
        1 + self.cap_regs.hcs_params_1().read().max_slots() as usize
    }

    fn set_up_dcbaa(&mut self, allocations: [MultiAllocResponse; 3]) -> Dcbaa {
        let [
            dcbaa_allocations,
            scratchpad_array_allocations,
            scratchpad_buffer_allocations,
        ] = allocations;

        // We need 1 slot for the scratchpad array, and 1 slot for each possible device
        let dcbaa_len = 1 + self.cap_regs.hcs_params_1().read().max_slots() as usize;
        let mut dcbaa_ptr = NonNull::new(slice_from_raw_parts_mut(
            dcbaa_allocations[0].virt_addr.get() as *mut u64,
            dcbaa_len,
        ))
        .expect("ptr is not null");
        // Safety: The caller promised that the virt addr points to the page
        let dcbaa = unsafe { dcbaa_ptr.as_mut() };
        // Initialize the dcbaa with 0s
        dcbaa.fill(0);
        let mut driver_virt_dcbaa = vec![None; dcbaa_len].into_boxed_slice();

        if let Some(required_scratchpad_pages) = NonZero::new(self.required_scratchpad_buffers()) {
            let mut scratchpad_array_ptr = NonNull::new(slice_from_raw_parts_mut(
                scratchpad_array_allocations[0].virt_addr.get() as *mut u64,
                required_scratchpad_pages.get() as usize,
            ))
            .expect("ptr is not null");
            // Safety: the scratchpad array is 1 page, and the scratchpad array fits in 1 page
            let scratchpad_array = unsafe { scratchpad_array_ptr.as_mut() };
            for i in 0..required_scratchpad_pages.get() as usize {
                scratchpad_array[i] = scratchpad_buffer_allocations[i].phys_addr;
            }

            // The first dcbaa item is a pointer to the scratchpads array
            dcbaa[0] = scratchpad_array_allocations[0].phys_addr;
            driver_virt_dcbaa[0] = Some(scratchpad_array_allocations[0].virt_addr);
        }

        self.op_regs().dcbaap().update(|mut dcbaap| {
            dcbaap.set_dcbaap(dcbaa_allocations[0].phys_addr);
            dcbaap
        });

        driver_virt_dcbaa
    }

    fn configure_runtime_registers_req(&self) -> [Option<MultiAllocRequest>; 2] {
        XhciEventRing::new_req(XHCI_EVENT_RING_TRB_COUNT)
    }

    fn configure_runtime_registers(&mut self, res: &[MultiAllocResponse; 2]) {
        // Get the primary interrupter registers
        let interrupter_regs = self
            .runtime_regs()
            .interrupter_register_sets()
            .as_slice()
            .index(0);

        // Enable interrupts
        interrupter_regs.iman().update(|mut iman| {
            iman.set_interrupt_enable(true);
            iman
        });

        // Set up the event ring and write to the interrupter
        // Registers to set: ERSTSZ, ERDP, ERSTBA
        let xhci_event_ring = XhciEventRing::new(XHCI_EVENT_RING_TRB_COUNT, res, interrupter_regs);
        log::debug!("ERSTSZ : {:#X?}", interrupter_regs.erstsz().read());
        log::debug!("ERSTBA : {:#X?}", interrupter_regs.erstba().read());
        log::debug!("ERDP   : {:#X?}", interrupter_regs.erdp().read());
        self.event_ring = Some(xhci_event_ring);

        // Clear any pending interrupts for the primary interrupter
        self.acknowledge_irq(0);
    }

    fn acknowledge_irq(&mut self, interrupter: u16) {
        self.op_regs().usb_sts().write({
            let mut usb_sts = UsbSts(0);
            usb_sts.set_eint(true);
            usb_sts
        });

        let interrupter_regs = self
            .runtime_regs()
            .interrupter_register_sets()
            .as_slice()
            .index(interrupter as usize);

        interrupter_regs.iman().update(|mut iman| {
            // To set this to false, you actually write true to this
            iman.set_interrupt_pending(true);
            iman
        });
    }
}
