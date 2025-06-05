use core::slice;

use bincode::{Decode, Encode};

#[derive(Clone, Copy, Debug, Encode, Decode, PartialEq, Eq)]
pub struct SliceData {
    pointer: u64,
    /// The len of whatever type is being represented, not necessarily the number of `u8`s
    len: u64,
}

impl SliceData {
    pub fn from_slice<T>(slice: &[T]) -> Self {
        Self {
            pointer: slice.as_ptr() as u64,
            len: slice.len() as u64,
        }
    }

    /// # Safety
    /// See `core::slice::from_raw_parts`
    pub unsafe fn to_slice<'a, T>(&self) -> &'a [T] {
        unsafe { slice::from_raw_parts(self.pointer as *const _, self.len as usize) }
    }

    /// # Safety
    /// See `core::slice::from_raw_parts`
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
