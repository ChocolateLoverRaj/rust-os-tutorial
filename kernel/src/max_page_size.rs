use common::PageSize;
use raw_cpuid::CpuId;

pub fn max_page_size() -> PageSize {
    if CpuId::new()
        .get_extended_processor_and_feature_identifiers()
        .is_some_and(|info| info.has_1gib_pages())
    {
        PageSize::_1GiB
    } else {
        PageSize::_2MiB
    }
}
