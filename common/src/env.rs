#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct EnvEntry {
    /// Generate a random u64 to avoid conflicts
    pub key: u64,
    /// Use this as data or as a pointer, it's up to you
    pub value: u64,
}
