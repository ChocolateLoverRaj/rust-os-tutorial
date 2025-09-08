use num_enum::{IntoPrimitive, TryFromPrimitive};

#[non_exhaustive]
#[repr(u32)]
#[derive(Debug, TryFromPrimitive, IntoPrimitive, PartialEq, Eq)]
pub enum ElfSegmentType {
    Load = 0x1,
}
