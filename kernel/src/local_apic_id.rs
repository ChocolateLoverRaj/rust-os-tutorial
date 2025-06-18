use limine::response::MpResponse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalApicId(u32);

impl LocalApicId {
    pub fn bsp(mp_response: &'static MpResponse) -> Self {
        Self(mp_response.bsp_lapic_id())
    }
}

impl From<&limine::mp::Cpu> for LocalApicId {
    fn from(value: &limine::mp::Cpu) -> Self {
        Self(value.lapic_id)
    }
}

impl From<LocalApicId> for u32 {
    fn from(value: LocalApicId) -> Self {
        value.0
    }
}
