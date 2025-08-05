use core::{num::NonZero, ptr::NonNull, slice};

use bincode::{Decode, Encode};
use zerocopy::{FromBytes, Immutable, KnownLayout, TryFromBytes};

pub use zerocopy;

#[derive(Clone, Copy, Debug, Encode, Decode, PartialEq, Eq, FromBytes, Immutable)]
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

#[derive(
    Clone, Copy, Debug, Encode, Decode, PartialEq, Eq, Immutable, TryFromBytes, KnownLayout,
)]
#[repr(C)]
pub struct SliceData2 {
    pub ptr: NonZero<usize>,
    pub len: usize,
}

impl SliceData2 {
    pub fn from_slice<T>(slice: &[T]) -> Self {
        SliceData2 {
            ptr: NonNull::from_ref(slice).addr(),
            len: slice.len(),
        }
    }
}
