use core::sync::atomic::AtomicPtr;

use bitfield::bitfield;
use num_enum::IntoPrimitive;
use volatile::VolatilePtr;

pub type DoorbellArray = [DoorbellRegister; 256];

bitfield! {
    /// xHCI 5.6 Doorbell Registers
    #[derive(Clone, Copy)]
    pub struct DoorbellRegister(u32);
    impl Debug;

    u8; pub db_target, set_db_target: 7, 0;
    u16; pub db_stream_id, set_db_stream_id: 31, 16;
}

#[derive(Debug, IntoPrimitive)]
#[repr(u8)]
pub enum HostControllerDoorbellTarget {
    CommandDoorbell,
}

pub struct DoorbellManager;

impl DoorbellManager {
    pub fn ring_doorbell(doorbell_array: VolatilePtr<DoorbellArray>, doorbell: u8, target: u8) {
        doorbell_array.as_slice().index(doorbell as usize).write({
            let mut reg = DoorbellRegister(0);
            reg.set_db_target(target);
            reg
        });
    }

    pub fn ring_command_doorbell(doorbell_array: VolatilePtr<DoorbellArray>) {
        Self::ring_doorbell(
            doorbell_array,
            0,
            HostControllerDoorbellTarget::CommandDoorbell.into(),
        );
    }
}
