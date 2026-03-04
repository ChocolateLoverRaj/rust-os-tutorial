use arbitrary_int::{u20, u22};
use bitbybit::bitfield;
use loader::paging::Paging;

pub struct RiscvPaging;
impl Paging for RiscvPaging {}
