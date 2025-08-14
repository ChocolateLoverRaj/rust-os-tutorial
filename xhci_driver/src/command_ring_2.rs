use core::{
    mem::MaybeUninit,
    num::NonZero,
    ptr::{NonNull, slice_from_raw_parts_mut},
};

use crate::*;

/// 4.9 TRB Ring
#[derive(Debug)]
pub struct CommandRing2<'a> {
    ring_mem: AllocResponse,
    ring: &'a mut [TransferRequestBlock],
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
    pub fn new(len: usize, allocate: impl Fn(AllocRequest) -> AllocResponse) -> Self {
        let command_ring_len = len;
        let command_ring_size = command_ring_len * size_of::<TransferRequestBlock>();
        let command_ring_mem = allocate(AllocRequest {
            size: NonZero::new(command_ring_size as u64).unwrap(),
            align: XHCI_COMMAND_RING_SEGMENTS_ALIGNMENT,
            boundary: XHCI_COMMAND_RING_SEGMENTS_BOUNDARY,
        });
        let command_ring = {
            {
                let mut ptr = NonNull::new(slice_from_raw_parts_mut(
                    command_ring_mem.virt_addr.get() as *mut MaybeUninit<TransferRequestBlock>,
                    command_ring_len,
                ))
                .unwrap();
                let command_ring_uninit = unsafe { ptr.as_mut() };
                // Initially when the TRB Ring is created in memory, or if it is ever re -initialized, all TRBs in the ring shall be cleared to ‘0’. This state represents an empty queue.
                command_ring_uninit.fill(MaybeUninit::zeroed());
            }
            let mut ptr = NonNull::new(slice_from_raw_parts_mut(
                command_ring_mem.virt_addr.get() as *mut TransferRequestBlock,
                command_ring_len,
            ))
            .unwrap();
            unsafe { ptr.as_mut() }
        };
        let initial_cycle_state = true;
        // Make the last TRB a link TRB
        *command_ring.last_mut().unwrap() = TransferRequestBlock {
            // Point to first TRB slot
            parameter: command_ring_mem.phys_addr,
            status: 0,
            control: {
                let mut control = TrbControl(0);
                control.set_cycle_bit(initial_cycle_state);
                control.set_toggle_cycle(true);
                control.set_trb_type(XhciTrbType::Link.into());
                control
            },
        };
        Self {
            ring_mem: command_ring_mem,
            ring: command_ring,
            enqueue_pointer: 0,
            producer_cycle_state: initial_cycle_state,
            dequeue_pointer: 0,
            consumer_cycle_state: initial_cycle_state,
        }
    }

    pub fn phys_addr(&self) -> u64 {
        self.ring_mem.phys_addr
    }

    pub fn producer_cycle_state(&self) -> bool {
        self.producer_cycle_state
    }

    /// The cycle bit will be set by this function
    pub fn try_enqueue(&mut self, mut trb: TransferRequestBlock) -> Result<(), EnqueueError> {
        let can_enqueue = if self.consumer_cycle_state == self.producer_cycle_state {
            self.enqueue_pointer >= self.dequeue_pointer
        } else {
            self.enqueue_pointer < self.dequeue_pointer
        };
        if can_enqueue {
            trb.control.set_cycle_bit(self.producer_cycle_state);
            self.ring[self.enqueue_pointer] = trb;
            Ok(())
        } else {
            Err(EnqueueError::IsFull)
        }
    }
}

#[derive(Debug)]
pub enum EnqueueError {
    IsFull,
}
