use bitfield::bitfield;
use zerocopy::{FromBytes, Immutable, IntoBytes, transmute};

use crate::{trb::AnyTrb, trb_type::XhciTrbType};

/// xHCI 6.4.2.2 Command Completion Event TRB
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Immutable)]
pub struct XhciCommandCompletionEventTrb {
    pub command_trb_pointer: CommandTrbPointer,
    pub status: CommandCompletionEventStatus,
    pub control: CommandCompletionEventControl,
}

#[derive(Debug)]
pub enum TrbConvertError {
    WrongType(u8),
}

impl TryFrom<AnyTrb> for XhciCommandCompletionEventTrb {
    type Error = TrbConvertError;
    fn try_from(value: AnyTrb) -> Result<Self, Self::Error> {
        let trb_type = value.control.trb_type();
        if trb_type == XhciTrbType::CmdCompletionEvent.into() {
            Ok(transmute!(value))
        } else {
            Err(TrbConvertError::WrongType(trb_type))
        }
    }
}

bitfield! {
    #[derive(Clone, Copy, FromBytes, IntoBytes, Immutable)]
    pub struct CommandTrbPointer(u64);
    impl Debug;

    u64; _command_trb_pointer, _: 63, 4;
}

impl CommandTrbPointer {
    pub fn command_trb_pointer(&self) -> u64 {
        self._command_trb_pointer() << 4
    }
}

bitfield! {
    #[derive(Clone, Copy, FromBytes, IntoBytes, Immutable)]
    pub struct CommandCompletionEventStatus(u32);
    impl Debug;

    u32;
    /// This field may optionally be set by a command. Refer to section 4.6.6.1 for specific usage. If a command does not utilize this field it shall be treated as RsvdZ.
    pub command_completion_parameter, _: 23, 0;
    u8;
    /// This field encodes the completion status of the command that generated the event.
    /// Refer to the respective command definition for a list of the possible Completion Codes associated with the command.
    /// Refer to section 6.4.5 for an enumerated list of possible error conditions.
    pub completion_code, _: 31, 24;
}

bitfield! {
    #[derive(Clone, Copy, FromBytes, IntoBytes, Immutable)]
    pub struct CommandCompletionEventControl(u32);
    impl Debug;

    pub cycle_bit, _: 0;
    u8; pub trb_type, _: 15, 10;
    u8; pub virtual_function_id, _: 23, 16;
    u8; pub slot_id, _: 31, 24;
}
