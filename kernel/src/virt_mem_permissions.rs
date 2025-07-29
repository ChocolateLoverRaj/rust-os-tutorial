use common::{ElfSegmentFlags, PermissionFlags, SpawnProcessMemoryFlags};
use x86_64::structures::paging::PageTableFlags;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct VirtMemPermissions {
    /// If this is fault, this means that the page is intentionally left unmapped.
    /// This is useful for stack guard pages
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl VirtMemPermissions {
    pub fn page_table_flags(&self) -> PageTableFlags {
        let mut flags = PageTableFlags::empty();
        if self.read {
            flags |= PageTableFlags::PRESENT;
        }
        if self.write {
            flags |= PageTableFlags::WRITABLE;
        }
        if !self.execute {
            flags | PageTableFlags::NO_EXECUTE;
        }
        flags
    }
}

impl From<ElfSegmentFlags> for VirtMemPermissions {
    fn from(value: ElfSegmentFlags) -> Self {
        Self {
            read: value.contains(ElfSegmentFlags::READABLE),
            write: value.contains(ElfSegmentFlags::WRITABLE),
            execute: value.contains(ElfSegmentFlags::EXECUTABLE),
        }
    }
}

impl From<SpawnProcessMemoryFlags> for VirtMemPermissions {
    fn from(value: SpawnProcessMemoryFlags) -> Self {
        Self {
            read: value.contains(SpawnProcessMemoryFlags::READABLE),
            write: value.contains(SpawnProcessMemoryFlags::WRITABLE),
            execute: value.contains(SpawnProcessMemoryFlags::EXECUTABLE),
        }
    }
}

impl From<PermissionFlags> for VirtMemPermissions {
    fn from(value: PermissionFlags) -> Self {
        Self {
            read: value.contains(PermissionFlags::READABLE),
            write: value.contains(PermissionFlags::WRITABLE),
            execute: value.contains(PermissionFlags::EXECUTABLE),
        }
    }
}
