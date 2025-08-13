use core::{
    mem::MaybeUninit,
    num::NonZero,
    ptr::{NonNull, slice_from_raw_parts_mut},
};

use crate::*;

#[derive(Debug)]
pub struct XhciCommandRing {
    mem: AllocResponse,
    max_trbs: NonZero<usize>,
    position: usize,
    ring_cycle_state: bool,
}

impl XhciCommandRing {
    pub fn new_req(max_trbs: NonZero<usize>) -> AllocRequest {
        AllocRequest {
            size: NonZero::new((max_trbs.get() * size_of::<TransferRequestBlock>()) as u64)
                .expect("size > 0"),
            align: XHCI_COMMAND_RING_SEGMENTS_ALIGNMENT,
            boundary: XHCI_COMMAND_RING_SEGMENTS_BOUNDARY,
        }
    }

    /// Mem must be valid
    pub fn new(max_trbs: NonZero<usize>, mem: AllocResponse) -> Self {
        let mut trbs_ptr = NonNull::new(slice_from_raw_parts_mut(
            mem.virt_addr.get() as *mut MaybeUninit<TransferRequestBlock>,
            max_trbs.get(),
        ))
        .unwrap();
        // # Safety: caller guaranteed that the mem is valid
        let trbs = unsafe { trbs_ptr.as_mut() };
        // Initialize slots with 0
        trbs.fill(MaybeUninit::new(TransferRequestBlock {
            parameter: 0,
            status: 0,
            control: TrbControl(0),
        }));

        // The tutorial sets this to 1 initially so we will too
        // This is probably because by default the cycle state on all of the zeroed slots is 0
        let ring_cycle_state = true;

        // Set the last TRB to point back to the first one
        trbs.last_mut()
            .expect("at least 1 trb")
            .write(TransferRequestBlock {
                parameter: mem.phys_addr,
                status: 0,
                control: {
                    let mut control = TrbControl(0);
                    control.set_trb_type(XhciTrbType::Link.into());
                    control.set_cycle_bit(ring_cycle_state);
                    control.set_toggle_cycle(true);
                    control
                },
            });
        Self {
            mem,
            max_trbs,
            position: 0,
            ring_cycle_state,
        }
    }

    pub fn phys_addr(&self) -> u64 {
        self.mem.phys_addr
    }

    pub fn cycle_bit(&self) -> bool {
        self.ring_cycle_state
    }

    /// The cycle bit will be modified by this fn to be the correct one
    pub fn enqueue(&mut self, mut trb: TransferRequestBlock) {
        let mut trbs_ptr = NonNull::new(slice_from_raw_parts_mut(
            self.mem.virt_addr.get() as *mut TransferRequestBlock,
            self.max_trbs.get(),
        ))
        .unwrap();
        // Safety: mem is valid and initialized
        let trbs = unsafe { trbs_ptr.as_mut() };

        trb.control.set_cycle_bit(self.ring_cycle_state);

        trbs[self.position] = trb;

        self.position += 1;
        if self.position == self.max_trbs.get() - 1 {
            trbs.last_mut()
                .expect("trbs > 1")
                .control
                .set_cycle_bit(self.ring_cycle_state);
            self.position = 0;
            self.ring_cycle_state = !self.ring_cycle_state;
        }
    }
}
