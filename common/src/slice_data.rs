use core::slice;

use bincode::{Decode, Encode};
use zerocopy::{Immutable, TryFromBytes};

pub use zerocopy;

#[derive(Clone, Copy, Debug, Encode, Decode, PartialEq, Eq)]
pub struct SliceData {
    pointer: u64,
    /// The len of whatever type is being represented, not necessarily the number of `u8`s
    len: u64,
}

impl SliceData {
    pub fn new(pointer: u64, len: u64) -> Self {
        Self { pointer, len }
    }

    /// # Safety
    /// See [`core::slice::from_raw_parts`]
    pub unsafe fn to_slice<'a, T>(&self) -> &'a [T] {
        unsafe { slice::from_raw_parts(self.pointer as *const _, self.len as usize) }
    }

    /// # Safety
    /// See [`core::slice::from_raw_parts`]
    pub unsafe fn try_to_slice<'a, T: TryFromBytes + Immutable>(&self) -> Option<&'a [T]> {
        let slice = unsafe {
            slice::from_raw_parts(
                self.pointer as *const u8,
                self.len as usize * size_of::<T>(),
            )
        };
        zerocopy::TryFromBytes::try_ref_from_bytes(slice).ok()
    }

    /// Treats the slice as `&[T]`, but creates `&[u8]` to not assume valid alignment
    ///
    /// # Safety
    /// See [`core::slice::from_raw_parts`]
    pub unsafe fn to_slice_bytes<'a, T>(&self) -> &'a [u8] {
        unsafe {
            slice::from_raw_parts(self.pointer as *const _, self.len as usize * size_of::<T>())
        }
    }

    /// # Safety
    /// See [`core::slice::from_raw_parts_mut`]
    pub unsafe fn to_slice_mut<'a, T>(&self) -> &'a mut [T] {
        unsafe { slice::from_raw_parts_mut(self.pointer as *mut _, self.len as usize) }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn pointer(&self) -> u64 {
        self.pointer
    }
}

impl<T> From<&[T]> for SliceData {
    fn from(value: &[T]) -> Self {
        Self {
            pointer: value.as_ptr() as u64,
            len: value.len() as u64,
        }
    }
}

impl<T> From<&mut [T]> for SliceData {
    fn from(value: &mut [T]) -> Self {
        Self {
            pointer: value.as_ptr() as u64,
            len: value.len() as u64,
        }
    }
}
