use core::convert::Infallible;

use atomic_enum::atomic_enum;
use common::embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, DrawTarget, Point, Size},
    primitives::Rectangle,
};

pub const ID: u64 = 56;

#[repr(C)]
pub struct WindowInfo {
    pub width: u64,
    pub height: u64,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
}

#[atomic_enum]
#[derive(PartialEq)]
enum ActiveSlot {
    Slot0,
    Slot1,
}

#[repr(C)]
struct WindowSharedMem {
    request_channel_id: u64,
    response_channel_id: u64,
    window_info: WindowInfo,
    copy_data: CopyData,
    // The client can write to the other slot while the server reads from one slot
    // active_slot: AtomicActiveSlot,
    // Two copies of pixels go below this
    // pixels: [u32],
    // data: [u8; 0x1000 - (size_of::<u64>() + size_of::<u64>() + size_of::<WindowInfo>())],
}

#[repr(C)]
struct CopyData {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

pub struct WindowsSharedMemClient {
    data: &'static mut WindowSharedMem,
}

impl WindowsSharedMemClient {
    pub unsafe fn new(data: u64) -> Self {
        let ptr = data as *mut WindowSharedMem;
        let data = unsafe { ptr.as_mut() }.unwrap();
        Self { data }
    }
}

impl DrawTarget for WindowsSharedMemClient {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = common::embedded_graphics::Pixel<Self::Color>>,
    {
        Ok(())
    }
}

impl Dimensions for WindowsSharedMemClient {
    fn bounding_box(&self) -> common::embedded_graphics::primitives::Rectangle {
        Rectangle::new(
            Point::zero(),
            Size::new(
                self.data.window_info.width.try_into().unwrap(),
                self.data.window_info.height.try_into().unwrap(),
            ),
        )
    }
}
