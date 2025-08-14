use crate::{
    trb::{AnyTrb, AnyTrbControl},
    trb_type::XhciTrbType,
};

pub fn enable_slot_command_trb() -> AnyTrb {
    AnyTrb {
        parameter: 0,
        status: 0,
        control: {
            let mut control = AnyTrbControl(0);
            control.set_trb_type(XhciTrbType::EnableSlotCmd.into());
            control
        },
    }
}
