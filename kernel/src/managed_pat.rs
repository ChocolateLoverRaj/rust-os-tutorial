use ez_paging::{ManagedPat, PagingConfig, VirtualOffset};
use spin::Lazy;

use crate::hhdm_offset::HhdmOffset;

pub const PAGING_CONFIG: Lazy<PagingConfig> = Lazy::new(|| {
    PagingConfig::new(
        // Safety: We do not ever modify the PAT MSR. Limine sets it and it doesn't get modified after that.
        unsafe { ManagedPat::new() },
        {
            let offset = HhdmOffset::get_from_response().into();
            unsafe { VirtualOffset::new(offset) }
        },
    )
});
