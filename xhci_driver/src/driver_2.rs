use core::{
    mem::MaybeUninit,
    num::NonZero,
    ptr::{NonNull, slice_from_raw_parts_mut},
};

use volatile::VolatileRef;

use crate::*;

pub struct Driver2<'a> {
    capability_regs: VolatileRef<'a, CapabilityRegs>,
    operational_regs: VolatileRef<'a, OperationalRegs>,
    runtime_regs: VolatileRef<'a, RuntimeRegisters>,
    doorbell_regs: VolatileRef<'a, DoorbellArray>,
    command_ring: CommandRing2<'a>,
    event_ring: EventRing2<'a>,
}

impl Driver2<'_> {
    /// Remember to have disable interrupts while this function is executing.
    /// Otherwise you could get an xHCI interrupt and cause a deadlock.
    ///
    /// # Safety
    /// You must find the address and size of BAR 0, and then create a mapping for the entire BAR 0 with the correct memory type (caching behavior).
    /// Then you input the virtual address that points to the start of BAR 0. The BAR should be an xHCI device's BAR.
    pub unsafe fn new(
        bar0: NonZero<usize>,
        allocate: impl Fn(AllocRequest) -> AllocResponse,
    ) -> Self {
        let capability_regs = {
            let capability_regs_ptr = NonNull::new(bar0.get() as *mut CapabilityRegs).unwrap();
            unsafe { VolatileRef::new(capability_regs_ptr) }
        };
        log::debug!(
            "Capability registers: {:#X?}",
            capability_regs.as_ptr().read()
        );
        let mut operational_regs = {
            let ptr = NonNull::new(
                (bar0.get() + capability_regs.as_ptr().cap_length().read() as usize)
                    as *mut OperationalRegs,
            )
            .unwrap();
            unsafe { VolatileRef::new(ptr) }
        };
        let mut runtime_regs = {
            let ptr = NonNull::new(
                (bar0.get() + capability_regs.as_ptr().rts_off().read() as usize)
                    as *mut RuntimeRegisters,
            )
            .unwrap();
            unsafe { VolatileRef::new(ptr) }
        };
        let mut doorbell_regs = {
            let ptr = NonNull::new(
                (bar0.get() + capability_regs.as_ptr().doorbell_offset().read() as usize)
                    as *mut DoorbellArray,
            )
            .unwrap();
            unsafe { VolatileRef::new(ptr) }
        };

        // Before we initialize the host controller, we will reset it
        operational_regs
            .as_mut_ptr()
            .usb_cmd()
            .update(|mut usb_cmd| {
                usb_cmd.set_host_controller_reset(true);
                usb_cmd
            });
        // Wait until reset is done
        loop {
            if !operational_regs
                .as_ptr()
                .usb_cmd()
                .read()
                .host_controller_reset()
                && !operational_regs
                    .as_ptr()
                    .usb_sts()
                    .read()
                    .controller_not_ready()
            {
                break;
            }
        }
        log::debug!("xHCI - Reset host controller");

        // xHCI 4 Operational Model
        // xHCI 4.2 Host Controller Initialization
        // Program the Max Device Slots Enabled (MaxSlotsEn) field in the CONFIG register (5.4.7) to enable the device slots that system software is going to use.
        let max_slots = capability_regs.as_ptr().hcs_params_1().read().max_slots();
        operational_regs.as_mut_ptr().config().update(|mut config| {
            config.set_max_slots_en(max_slots);
            config
        });

        // 6.1 Device Context Base Address Array
        // The Device Context Base Address Array shall contain MaxSlotsEn + 1 entries.
        let dcbaa_len = max_slots as usize + 1;
        let dcbaa_size = dcbaa_len * size_of::<u64>();
        let dcbaa_mem = allocate(AllocRequest {
            size: NonZero::new(dcbaa_size as u64).unwrap(),
            align: XHCI_DEVICE_CONTEXT_ALIGNMENT,
            boundary: XHCI_DEVICE_CONTEXT_BOUNDARY,
        });
        // System software initializes the Device Context Base Address Array to ‘0’, and updates individual entries when the respective Device Slot is allocated. The xHC reads an entry in the Device Context after a doorbell associated with the entries’ Device Slot is rung.
        let dcbaa = {
            let mut ptr = NonNull::new(slice_from_raw_parts_mut(
                dcbaa_mem.virt_addr.get() as *mut MaybeUninit<u64>,
                dcbaa_len,
            ))
            .unwrap();
            unsafe { ptr.as_mut() }
        };
        dcbaa.fill(MaybeUninit::zeroed());

        // If the Max Scratchpad Buffers field of the HCSPARAMS2 register is > ‘0’, then the first entry (entry_0) in the DCBAA shall contain a pointer to the Scratchpad Buffer Array.
        // If the Max Scratchpad Buffers field of the HCSPARAMS2 register is = ‘0’, then the first entry (entry_0) in the DCBAA is reserved and shall be cleared to ‘0’ by software.
        // TODO: Scratchpad
        assert_eq!(
            capability_regs
                .as_ptr()
                .hcs_params_2()
                .read()
                .max_scratchpad_buffers(),
            0
        );

        // Program the Device Context Base Address Array Pointer (DCBAAP) register (5.4.6) with a 64-bit address pointing to where the Device Context Base Address Array is located.
        operational_regs.as_mut_ptr().dcbaap().update(|mut dcbaap| {
            dcbaap.set_dcbaap(dcbaa_mem.phys_addr);
            dcbaap
        });

        // Define the Command Ring Dequeue Pointer by programming the Command Ring Control Register (5.4.5) with a 64-bit address pointing to the starting address of the first TRB of the Command Ring.
        let mut command_ring =
            CommandRing2::new(256, operational_regs.as_mut_ptr().crcr(), &allocate);

        // Initialize each active interrupter by:
        // Defining the Event Ring: (refer to section 4.9.4 for a discussion of Event Ring Management.)
        // Software maintains an Event Ring Consumer Cycle State (CCS) bit, initializing it to ‘1’ and toggling it every time the Event Ring Dequeue Pointer wraps back to the beginning of the Event Ring.

        // Allocate and initialize the Event Ring Segment(s).
        let mut event_ring = EventRing2::new(256, &allocate);

        // Allocate the Event Ring Segment Table (ERST) (section 6.5).
        // To keep things simple we'll only have a single segment
        let event_ring_segment_table_len = 1;
        let event_ring_segment_table_size =
            event_ring_segment_table_len * size_of::<XhciErstEntry>();
        let event_ring_segment_table_mem = allocate(AllocRequest {
            size: NonZero::new(event_ring_segment_table_size as u64).unwrap(),
            align: XHCI_EVENT_RING_SEGMENT_TABLE_ALIGNMENT,
            boundary: XHCI_EVENT_RING_SEGMENT_TABLE_BOUNDARY,
        });
        let event_ring_segment_table = {
            let mut ptr = NonNull::new(slice_from_raw_parts_mut(
                event_ring_segment_table_mem.virt_addr.get() as *mut MaybeUninit<XhciErstEntry>,
                event_ring_segment_table_len,
            ))
            .unwrap();
            unsafe { ptr.as_mut() }
        };
        // Initialize ERST table entries to point to and to define the size (in TRBs) of the respective Event Ring Segment.
        event_ring_segment_table[0].write(XhciErstEntry {
            ring_segment_base_address: event_ring.phys_addr(),
            ring_segment_size: event_ring.len() as u16,
            _reserved_0: [0; 6],
        });

        // Program the Interrupter Event Ring Segment Table Size (ERSTSZ) register (5.5.2.3.1) with the number of segments described by the Event Ring Segment Table.
        runtime_regs
            .as_mut_ptr()
            .interrupter_register_sets()
            .as_slice()
            .index(0)
            .erstsz()
            .update(|mut erstsz| {
                erstsz.set_erstsz(event_ring_segment_table_len as u16);
                erstsz
            });

        // Program the Interrupter Event Ring Dequeue Pointer (ERDP) register (5.5.2.3.3) with the starting address of the first segment described by the Event Ring Segment Table.
        event_ring.update_erdp(
            runtime_regs
                .as_mut_ptr()
                .interrupter_register_sets()
                .as_slice()
                .index(0)
                .erdp(),
        );

        // Program the Interrupter Event Ring Segment Table Base Address (ERSTBA) register (5.5.2.3.2) with a 64-bit address pointer to where the Event Ring Segment Table is located.
        // Note that writing the ERSTBA enables the Event Ring. Refer to section 4.9.4 for more information on the Event Ring registers and their initialization.
        runtime_regs
            .as_mut_ptr()
            .interrupter_register_sets()
            .as_slice()
            .index(0)
            .erstba()
            .update(|mut erstba| {
                erstba.set_erstba(event_ring_segment_table_mem.phys_addr);
                erstba
            });

        // Defining the interrupts:
        // Enable the MSI-X interrupt mechanism by setting the MSI-X Enable flag in the MSI-X Capability Structure Message Control register (5.2.8.3).
        // It's not this driver's job to do this.
        // From my experience, QEMU can do either legacy PCI interrupts or MSI-X interrupts. Both work.
        // In theory on real hardware it must support MSI, MSI-X or both. Legacy interrupts may or may not work.

        // Initializing the Interval field of the Interrupt Moderation register (5.5.2.2) with the target interrupt moderation rate.
        // We can leave it as 0x0, which will not throttle interrupts at all

        // Enable system bus interrupt generation by writing a ‘1’ to the Interrupter Enable (INTE) flag of the USBCMD register (5.4.1).
        operational_regs
            .as_mut_ptr()
            .usb_cmd()
            .update(|mut usb_cmd| {
                usb_cmd.set_interrupter_enable(true);
                usb_cmd
            });

        // Enable the Interrupter by writing a ‘1’ to the Interrupt Enable (IE) field of the Interrupter Management register (5.5.2.1).
        runtime_regs
            .as_mut_ptr()
            .interrupter_register_sets()
            .as_slice()
            .index(0)
            .iman()
            .update(|mut iman| {
                iman.set_interrupt_enable(true);
                iman
            });

        // Write the USBCMD (5.4.1) to turn the host controller ON via setting the Run/Stop (R/S) bit to ‘1’. This operation allows the xHC to begin accepting doorbell references.
        operational_regs
            .as_mut_ptr()
            .usb_cmd()
            .update(|mut usb_cmd| {
                usb_cmd.set_run_stop(true);
                usb_cmd
            });

        // Send a command
        command_ring
            .try_enqueue(AnyTrb {
                parameter: 0,
                status: 0,
                control: {
                    let mut control = AnyTrbControl(0);
                    control.set_trb_type(XhciTrbType::EnableSlotCmd.into());
                    control
                },
            })
            .expect("the command ring is currently empty");
        command_ring
            .try_enqueue(AnyTrb {
                parameter: 0,
                status: 0,
                control: {
                    let mut control = AnyTrbControl(0);
                    control.set_trb_type(XhciTrbType::EnableSlotCmd.into());
                    control
                },
            })
            .expect("the command ring is currently empty");

        // Ring the doorbell
        doorbell_regs.as_mut_ptr().as_slice().index(0).write({
            let mut reg = DoorbellRegister(0);
            reg.set_db_target(HostControllerDoorbellTarget::CommandDoorbell.into());
            reg
        });

        log::debug!(
            "Operational registers: {:#X?}",
            operational_regs.as_ptr().read()
        );
        log::debug!(
            "Interrupter registers: {:#X?}",
            runtime_regs
                .as_mut_ptr()
                .interrupter_register_sets()
                .as_slice()
                .index(0)
                .read()
        );

        Self {
            capability_regs,
            operational_regs,
            runtime_regs,
            doorbell_regs,
            command_ring,
            event_ring,
        }
    }

    /// Again, remember to disable interrupts while executing this fn
    pub fn handle_interrupt(&mut self) {
        let events = self.event_ring.peek();
        // TODO: From the Command Completion Events, we can get the dequeue pointer of the command ring. Then we need to update our command ring dequeue pointer so it doesn't overflow.
        log::log!(log::Level::Debug, "New events: {:#X?}", events);
        self.event_ring.advance_dequeue_pointer(
            events.0.len() + events.1.len(),
            self.runtime_regs
                .as_mut_ptr()
                .interrupter_register_sets()
                .as_slice()
                .index(0)
                .erdp(),
        );
    }
}
