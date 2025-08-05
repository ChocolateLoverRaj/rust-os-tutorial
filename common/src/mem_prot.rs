use bitflags::bitflags;

bitflags! {
    /// If `READABLE` is not set, then no other flags can be set, because it doesn't make sense.
    #[derive(Debug, Clone, Copy)]
    pub struct MemProt: u8 {
        const EXECUTABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        const READABLE = 1 << 2;

        // The source may set any bits
        const _ = !0;
    }
}
