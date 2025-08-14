use core::{
    mem::MaybeUninit,
    num::NonZero,
    ptr::{NonNull, slice_from_raw_parts_mut},
};

use alloc::vec::Vec;
use volatile::{VolatileFieldAccess, VolatilePtr};

use crate::*;

/// xHCI 6.5 Event Ring Segment Table
#[derive(Debug, VolatileFieldAccess)]
#[repr(C)]
pub struct XhciErstEntry {
    pub ring_segment_base_address: u64,
    pub ring_segment_size: u16,
    pub _reserved_0: [u8; 6],
}

// Event ring will only use 1 segment because QEMU probably only supports 1 segment
const SEGMENT_COUNT: usize = 1;

#[derive(Debug)]
pub(crate) struct XhciEventRing {
    max_trbs: NonZero<u32>,
    ring_mem: AllocResponse,
    table_mem: AllocResponse,
    position: u32,
    ring_cycle_state: bool,
}

impl XhciEventRing {
    pub fn new_req(max_trbs: NonZero<u32>) -> [Option<MultiAllocRequest>; 2] {
        let segment_size = max_trbs.get() as usize * size_of::<TransferRequestBlock>();
        let segment_table_size = SEGMENT_COUNT * size_of::<XhciErstEntry>();

        [
            Some(MultiAllocRequest {
                request: AllocRequest {
                    size: NonZero::new(segment_size as u64).expect("size > 0"),
                    align: XHCI_EVENT_RING_SEGMENTS_ALIGNMENT,
                    boundary: XHCI_EVENT_RING_SEGMENTS_BOUNDARY,
                },
                count: NonZero::new(1).unwrap(),
            }),
            Some(MultiAllocRequest {
                request: AllocRequest {
                    size: NonZero::new(segment_table_size as u64).expect("size > 0"),
                    align: XHCI_EVENT_RING_SEGMENT_TABLE_ALIGNMENT,
                    boundary: XHCI_EVENT_RING_SEGMENT_TABLE_BOUNDARY,
                },
                count: NonZero::new(1).unwrap(),
            }),
        ]
    }

    pub fn new(
        max_trbs: NonZero<u32>,
        mem: &[MultiAllocResponse; 2],
        interrupter_regs: VolatilePtr<InterrupterRegs>,
    ) -> Self {
        let ring_mem = mem[0][0];
        let table_mem = mem[1][0];

        // Set up the event ring itself
        let mut event_ring_ptr = NonNull::new(slice_from_raw_parts_mut(
            ring_mem.virt_addr.get() as *mut MaybeUninit<TransferRequestBlock>,
            max_trbs.get() as usize,
        ))
        .unwrap();
        // Safety: mem is valid
        let event_ring = unsafe { event_ring_ptr.as_mut() };
        event_ring.fill(MaybeUninit::new(TransferRequestBlock {
            parameter: 0,
            status: 0,
            control: TrbControl(0),
        }));

        // Set up the event ring segment table
        let mut segment_table_ptr = NonNull::new(slice_from_raw_parts_mut(
            table_mem.virt_addr.get() as *mut MaybeUninit<XhciErstEntry>,
            SEGMENT_COUNT,
        ))
        .unwrap();
        // Safety: mem is valid
        let segment_table = unsafe { segment_table_ptr.as_mut() };
        let table_entry = segment_table[0].write(XhciErstEntry {
            ring_segment_base_address: ring_mem.phys_addr,
            ring_segment_size: max_trbs.get() as u16,
            _reserved_0: [0; 6],
        });

        log::debug!("Segment table: {table_entry:p}: {:#X?}", table_entry);

        let mut s: XhciEventRing = Self {
            max_trbs,
            ring_mem,
            table_mem,
            position: 0,
            ring_cycle_state: true,
        };

        // The order of the register configuring matters
        // Configure the Event Ring Segment Table Size (ERSTSZ) register
        interrupter_regs.erstsz().write({
            let mut erstsz = Erstsz(0);
            erstsz.set_erstsz(1);
            erstsz
        });

        // Initialize and set ERDP
        s.update_erdp(interrupter_regs);

        // Write to ERSTBA register
        interrupter_regs.erstba().write({
            let mut erstba = Erstba(0);
            erstba.set_erstba(table_mem.phys_addr);
            erstba
        });

        s
    }

    fn update_erdp(&mut self, interrupter_regs: VolatilePtr<InterrupterRegs>) {
        let dequeue_addr = self.ring_mem.phys_addr
            + (self.position as u64 * size_of::<TransferRequestBlock>() as u64);
        interrupter_regs.erdp().write({
            let mut erdp = Erdp(0);
            erdp.set_event_ring_dequeue_pointer(dequeue_addr);
            erdp
        });
    }

    fn ring_mut(&mut self) -> &mut [TransferRequestBlock] {
        let mut event_ring_ptr = NonNull::new(slice_from_raw_parts_mut(
            self.ring_mem.virt_addr.get() as *mut TransferRequestBlock,
            self.max_trbs.get() as usize,
        ))
        .unwrap();
        // Safety: mem is valid and initialized
        unsafe { event_ring_ptr.as_mut() }
    }

    fn dequeue_trb(&mut self) -> TransferRequestBlock {
        let index = self.position as usize;
        let trb = self.ring_mut()[index];
        if trb.control.cycle_bit() != self.ring_cycle_state {
            panic!("Event Ring attempted to dequeue an invalid TRB")
        }

        // Advance and possibly wrap the dequeue ptr if needed
        self.position += 1;
        if self.position == self.max_trbs.get() {
            self.position = 0;
            self.ring_cycle_state = !self.ring_cycle_state
        }

        trb
    }

    pub fn has_unprocessed_events(&mut self) -> bool {
        let index = self.position as usize;
        let trb = self.ring_mut()[index];
        trb.control.cycle_bit() == self.ring_cycle_state
    }

    fn dequeue_events(
        &mut self,
        interrupter_regs: VolatilePtr<InterrupterRegs>,
    ) -> Vec<TransferRequestBlock> {
        let mut events = Vec::new();

        // Process each event TRB
        while self.has_unprocessed_events() {
            events.push(self.dequeue_trb());
        }

        // Update the ERDP register
        self.update_erdp(interrupter_regs);

        // Clear the EHB (Event Handler Busy) bit
        interrupter_regs.erdp().update(|mut erdp| {
            // You clear it by writing true to it
            erdp.set_event_handler_busy(true);
            erdp
        });

        events
    }

    fn flush_unprocessed_events(&mut self, interrupter_regs: VolatilePtr<InterrupterRegs>) {
        self.dequeue_events(interrupter_regs);
    }
}
