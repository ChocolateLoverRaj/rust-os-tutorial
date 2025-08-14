use core::{
    mem::MaybeUninit,
    num::NonZero,
    ptr::{NonNull, slice_from_raw_parts_mut},
};

use volatile::VolatilePtr;
use zerocopy::transmute;

use crate::*;

/// 4.9 TRB Ring
#[derive(Debug)]
pub struct CommandRing2<'a> {
    _ring_mem: AllocResponse,
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
    pub fn new(
        len: usize,
        crcr: VolatilePtr<Crcr>,
        allocate: impl Fn(AllocRequest) -> AllocResponse,
    ) -> Self {
        let command_ring_len = len;
        let command_ring_size = command_ring_len * size_of::<AnyTrb>();
        let command_ring_mem = allocate(AllocRequest {
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
            _ring_mem: command_ring_mem,
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
                self.producer_cycle_state = !self.producer_cycle_state;
                self.ring[self.enqueue_pointer]
                    .control
                    .set_cycle_bit(self.producer_cycle_state);
                self.enqueue_pointer = 0;
            }

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
