use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct ElfSegmentFlags: u32 {
        const EXECUTABLE = 0b001;
        const WRITABLE = 0b010;
        const READABLE = 0b100;

        // The source may set any bits
        const _ = !0;
    }
}
