use core::ops::Sub;

use num_traits::{CheckedAdd, CheckedRem, Zero};

pub trait CheckedAlignUp: Sized {
    fn checked_align_up(self, multiple: Self) -> Option<Self>;
}

impl<T: Zero + CheckedRem + CheckedAdd + Eq + Sub<Output = T>> CheckedAlignUp for T {
    fn checked_align_up(self, multiple: Self) -> Option<Self> {
        Some({
            let rem = self.checked_rem(&multiple)?;
            if rem == Self::zero() {
                rem
            } else {
                self.checked_add(&(multiple - rem))?
            }
        })
    }
}

pub fn checked_align_up<T: Zero + CheckedRem + CheckedAdd + Eq + Sub<Output = T>>(
    n: T,
    multiple: T,
) -> Option<T> {
    Some({
        let rem = n.checked_rem(&multiple)?;
        if rem == T::zero() {
            n
        } else {
            n.checked_add(&(multiple - rem))?
        }
    })
}
