use common::AllocPageSize;
use raw_cpuid::CpuId;

pub fn max_page_size() -> AllocPageSize {
    if CpuId::new()
        .get_extended_processor_and_feature_identifiers()
        .is_some_and(|info| info.has_1gib_pages())
    {
        AllocPageSize::_1GiB
    } else {
        AllocPageSize::_2MiB
    }
}
