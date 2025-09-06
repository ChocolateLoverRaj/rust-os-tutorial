use ez_paging::ManagedL4PageTable;
use nodit::{Interval, NoditSet};

#[derive(Debug)]
pub struct VirtualMemory {
    #[allow(unused)]
    pub(super) set: NoditSet<u64, Interval<u64>>,
    #[allow(unused)]
    pub(super) l4: ManagedL4PageTable,
}
