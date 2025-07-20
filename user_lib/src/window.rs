use core::{convert::Infallible, mem::MaybeUninit, ptr::NonNull, slice};

use atomic_enum::atomic_enum;
use common::{
    AllocPageSize, FrameBufferEmbeddedGraphics, RgbPixelInfo,
    embedded_graphics::{
        Pixel,
        pixelcolor::Rgb888,
        prelude::{Dimensions, DrawTarget, Point, Size},
        primitives::Rectangle,
    },
};

use crate::{
    async_channel::{self, Receiver, Sender},
    syscall_alloc,
};

pub const ENV_KEY: u64 = 0x2994A6830F66D288;

#[repr(C)]
pub struct WindowInfo {
    pub width: u64,
    pub height: u64,
    pub pixel_info: RgbPixelInfo,
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
    // response_channel_id: u64,
    window_info: WindowInfo,
    copy_data: CopyData,
    // The client can write to the other slot while the server reads from one slot
    // active_slot: AtomicActiveSlot,
    // Two copies of pixels go below this
    // pixels: [u32],
    // data: [u8; 0x1000 - (size_of::<u64>() + size_of::<u64>() + size_of::<WindowInfo>())],
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct CopyData {
    pub x: u64,
    pub y: u64,
    pub width: u64,
    pub height: u64,
}

pub struct WindowSharedMemClient {
    data: &'static mut WindowSharedMem,
}

impl WindowSharedMemClient {
    pub unsafe fn new(data: u64) -> Self {
        let ptr = data as *mut WindowSharedMem;
        let data = unsafe { ptr.as_mut() }.unwrap();
        Self { data }
    }

    fn buffer(&mut self) -> &mut [u32] {
        let len = (self.data.window_info.width * self.data.window_info.height) as usize;
        let ptr = ((self.data as *mut _ as usize) + size_of::<WindowSharedMem>()) as *mut u32;
        unsafe { slice::from_raw_parts_mut(ptr, len) }
    }

    pub fn update_screen(&mut self, copy_data: CopyData) {
        self.data.copy_data = copy_data;
        let mut sender = unsafe { Sender::from_channel_id(self.data.request_channel_id) };
        sender.send();
    }
}

impl DrawTarget for WindowSharedMemClient {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = common::embedded_graphics::Pixel<Self::Color>>,
    {
        let width = self.data.window_info.width;
        let pixel_info = self.data.window_info.pixel_info;
        let buffer = self.buffer();
        for Pixel(point, color) in pixels {
            buffer[usize::try_from(point.y).unwrap() * usize::try_from(width).unwrap()
                + usize::try_from(point.x).unwrap()] = pixel_info.build_pixel(color);
        }
        Ok(())
    }
}

impl Dimensions for WindowSharedMemClient {
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

pub struct WindowSharedMemServer {
    shared_mem: NonNull<()>,
    receiver: Receiver,
    width: u64,
    height: u64,
}

#[derive(Debug)]
pub enum DrawToFrameBufferError {
    InvalidInput,
}

impl WindowSharedMemServer {
    pub fn new(width: u64, height: u64, frame_buffer: &FrameBufferEmbeddedGraphics) -> Self {
        let pixel_count = (width * height) as usize;
        let total_size = ((size_of::<WindowSharedMem>() + size_of::<u32>() * pixel_count) as u64)
            .next_multiple_of(AllocPageSize::_2MiB.size_bytes());
        let shared_mem =
            syscall_alloc(total_size.try_into().unwrap(), AllocPageSize::_2MiB).unwrap();
        let (sender, receiver) = async_channel::create();
        {
            let mut shared_mem = shared_mem.cast::<MaybeUninit<WindowSharedMem>>();
            let shared_mem = unsafe { shared_mem.as_mut() };
            shared_mem.write(WindowSharedMem {
                request_channel_id: sender.channel_id(),
                window_info: WindowInfo {
                    width,
                    height,
                    pixel_info: frame_buffer.info().pixel_info,
                },
                copy_data: Default::default(),
            });
        }
        Self {
            shared_mem: shared_mem.cast(),
            receiver,
            width,
            height,
        }
    }

    pub fn addr(&self) -> usize {
        self.shared_mem.addr().into()
    }

    pub fn size(&self) -> usize {
        let pixel_count = (self.width * self.height) as usize;
        (size_of::<WindowSharedMem>() + size_of::<u32>() * pixel_count)
            .next_multiple_of(AllocPageSize::_2MiB.size_bytes() as usize)
    }

    pub fn channel_id(&self) -> u64 {
        self.receiver.channel_id()
    }

    pub fn draw_to_frame_buffer(
        &mut self,
        frame_buffer: &mut FrameBufferEmbeddedGraphics,
        x: u64,
        y: u64,
    ) {
        let mut shared_mem_ptr = self.shared_mem.cast::<WindowSharedMem>();
        let shared_mem = unsafe { shared_mem_ptr.as_mut() };
        let pixels_ptr =
            (usize::from(self.shared_mem.addr()) + size_of::<WindowSharedMem>()) as *mut u32;
        let pixel_count = (self.width * self.height) as usize;
        let pixels = unsafe { slice::from_raw_parts_mut(pixels_ptr, pixel_count) };
        let copy_data = shared_mem.copy_data;
        // Restrict copy rect horizontal bounds
        if copy_data.x <= self.width {
            // Restrict copy rect vertical bounds
            for src_y in
                copy_data.y as usize..(copy_data.y + copy_data.height).min(self.height) as usize
            {
                let src_start_index = self.width as usize * src_y + copy_data.x as usize;
                let dest_start_index = frame_buffer.info().pitch as usize / size_of::<u32>()
                    * (y as usize + src_y)
                    + x as usize
                    + copy_data.x as usize;
                // Restrict copy rect horizontal bounds
                let copy_len = copy_data.width.min(self.width - copy_data.x) as usize;
                frame_buffer.buffer_mut()[dest_start_index..dest_start_index + copy_len]
                    .copy_from_slice(&pixels[src_start_index..src_start_index + copy_len]);
            }
        }
    }
}
