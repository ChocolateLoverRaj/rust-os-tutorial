use core::{
    mem::MaybeUninit,
    num::NonZero,
    ptr::{NonNull, slice_from_raw_parts_mut},
};

use volatile::VolatilePtr;
use zerocopy::{transmute, transmute_ref};

use crate::*;

/// 4.9 TRB Ring
#[derive(Debug)]
pub struct CommandRing2<'a> {
    ring_mem: AllocResponse,
    ring: &'a mut [AnyTrb],
    /// This is a position in the ring that we are at
    enqueue_pointer: usize,
    producer_cycle_state: bool,
    /// This is a position in the ring that the xHC is at
    /// We only update this when the xHC tells us.
    /// 4.9.3 Command Ring Management
    /// > The location of the Command Ring Dequeue Pointer is reported on the Event Ring in Command Completion Events.
    dequeue_pointer: usize,
    consumer_cycle_state: bool,
}

impl CommandRing2<'_> {
    /// Also updates CRCR
    pub fn new(len: usize, crcr: VolatilePtr<Crcr>, allocator: &mut impl XhciMemAllocator) -> Self {
        let command_ring_len = len;
        let command_ring_size = command_ring_len * size_of::<AnyTrb>();
        let command_ring_mem = allocator.alloc(AllocRequest {
            size: NonZero::new(command_ring_size as u64).unwrap(),
            align: XHCI_COMMAND_RING_SEGMENTS_ALIGNMENT,
            boundary: XHCI_COMMAND_RING_SEGMENTS_BOUNDARY,
        });
        let command_ring = {
            {
                let mut ptr = NonNull::new(slice_from_raw_parts_mut(
                    command_ring_mem.virt_addr.get() as *mut MaybeUninit<AnyTrb>,
                    command_ring_len,
                ))
                .unwrap();
                let command_ring_uninit = unsafe { ptr.as_mut() };
                // Initially when the TRB Ring is created in memory, or if it is ever re -initialized, all TRBs in the ring shall be cleared to ‘0’. This state represents an empty queue.
                command_ring_uninit.fill(MaybeUninit::zeroed());
            }
            let mut ptr = NonNull::new(slice_from_raw_parts_mut(
                command_ring_mem.virt_addr.get() as *mut AnyTrb,
                command_ring_len,
            ))
            .unwrap();
            unsafe { ptr.as_mut() }
        };
        let initial_cycle_state = true;
        // Make the last TRB a link TRB
        *command_ring.last_mut().unwrap() = transmute!(LinkTrb::new(
            command_ring_mem.phys_addr,
            initial_cycle_state,
            true
        ));

        crcr.update(|mut crcr| {
            crcr.set_command_ring_ptr(command_ring_mem.phys_addr);
            crcr.set_ring_cycle_state(initial_cycle_state);
            crcr
        });

        Self {
            ring_mem: command_ring_mem,
            ring: command_ring,
            enqueue_pointer: 0,
            producer_cycle_state: initial_cycle_state,
            dequeue_pointer: 0,
            consumer_cycle_state: initial_cycle_state,
        }
    }

    /// The cycle bit will be set by this function
    pub fn try_enqueue(&mut self, mut trb: AnyTrb) -> Result<(), EnqueueError> {
        let can_enqueue = if self.consumer_cycle_state == self.producer_cycle_state {
            self.enqueue_pointer >= self.dequeue_pointer
        } else {
            self.enqueue_pointer < self.dequeue_pointer
        };
        if can_enqueue {
            trb.control.set_cycle_bit(self.producer_cycle_state);
            self.ring[self.enqueue_pointer] = trb;

            self.enqueue_pointer += 1;
            if self.enqueue_pointer == self.ring.len() - 1 {
                // Update the producer cycle bit and also update the cycle bit in the Link TRB
                self.ring[self.enqueue_pointer]
                    .control
                    .set_cycle_bit(self.producer_cycle_state);
                self.producer_cycle_state = !self.producer_cycle_state;
                self.enqueue_pointer = 0;
            }

            Ok(())
        } else {
            Err(EnqueueError::IsFull)
        }
    }

    /// We can update the dequeue pointer based on events from the event ring.
    /// The command completion event tells us which command was completed.
    /// So we can advance the dequeue pointer based on the physical address of the command that the xHC finished.
    pub fn process_event(&mut self, event: &AnyTrb) {
        // xHCI 4.9.3 Command Ring Management
        // > The location of the Command Ring Dequeue Pointer is reported on the Event Ring in Command Completion Events.
        // xHCI 3.3 Command Interface
        // > Commands are executed by the xHC in the order that they are placed on the Command Ring.
        if event.control.trb_type() == XhciTrbType::CmdCompletionEvent.into() {
            let event: &XhciCommandCompletionEventTrb = transmute_ref!(event);
            let command_index = (event.command_trb_pointer.command_trb_pointer()
                - self.ring_mem.phys_addr) as usize
                / size_of::<AnyTrb>();
            // This could result in the dequeue pointer pointing to a Link TRB, which should be pretty instantly processed.
            // But we can't assume that the xHC processed the Link TRB and we shouldn't overwrite it until we're sure.
            // Since commands are executed in order, we don't need to worry about the dequeue pointer getting moved back because of out-of-order events.
            let new_dequeue_pointer = command_index + 1;
            log::debug!("Command ring dequeue pointer updated to {new_dequeue_pointer:X?}");
            // If the consumer (xHC) looped around, it must have toggled its consumer cycle state
            if new_dequeue_pointer < self.dequeue_pointer {
                self.consumer_cycle_state = !self.consumer_cycle_state;
            }
            self.dequeue_pointer = new_dequeue_pointer;
        }
    }
}

#[derive(Debug)]
pub enum EnqueueError {
    IsFull,
}
