use core::fmt::Arguments;

use crate::paging::Paging;

pub trait Arch {
    type Paging: Paging;

    fn early_log(arguments: Arguments<'_>);
    fn can_shutdown() -> bool;
    fn shutdown() -> !;
    fn low_power_loop() -> !;
}
