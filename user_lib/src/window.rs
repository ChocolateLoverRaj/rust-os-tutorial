use core::{
    convert::Infallible,
    mem::MaybeUninit,
    num::NonZero,
    ptr::NonNull,
    slice,
    sync::atomic::{AtomicBool, Ordering},
};

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
    EnvEntries, ExecutorContext,
    async_channel::{self, Receiver, Sender},
    syscall_alloc, syscall_clone_capability,
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
    pending_request: AtomicBool,
    request_channel_id: NonZero<u64>,
    window_info: WindowInfo,
    copy_data: CopyData,
    pixels: [u32; 0],
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
    /// # Safety
    /// This function should only be called once, because calling this function multiple times will result in multiple simultaneous mutable references.
    pub unsafe fn new(env_entries: &EnvEntries) -> Self {
        let ptr = *env_entries.get(&ENV_KEY).unwrap() as *mut WindowSharedMem;
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
        if !self.data.pending_request.swap(true, Ordering::Release) {
            sender.send();
        }
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

#[derive(Debug)]
pub struct WindowSharedMemServer {
    shared_mem: NonNull<[u8]>,
    receiver: Receiver,
    width: u64,
    height: u64,
}

#[derive(Debug)]
pub enum DrawToFrameBufferError {
    InvalidInput,
}

impl WindowSharedMemServer {
    pub fn new(
        width: u64,
        height: u64,
        frame_buffer: &FrameBufferEmbeddedGraphics,
    ) -> (Self, NonZero<u64>) {
        let pixel_count = (width * height) as usize;
        let total_size = ((size_of::<WindowSharedMem>() + size_of::<u32>() * pixel_count) as u64)
            .next_multiple_of(AllocPageSize::_2MiB.size_bytes());
        let shared_mem =
            syscall_alloc(total_size.try_into().unwrap(), AllocPageSize::_2MiB).unwrap();
        let (sender, receiver) = async_channel::create();
        let sender_capability = syscall_clone_capability(sender.channel_id()).unwrap();
        {
            let mut shared_mem = shared_mem.cast::<MaybeUninit<WindowSharedMem>>();
            let shared_mem = unsafe { shared_mem.as_mut() };
            shared_mem.write(WindowSharedMem {
                pending_request: Default::default(),
                request_channel_id: sender_capability,
                window_info: WindowInfo {
                    width,
                    height,
                    pixel_info: frame_buffer.info().pixel_info,
                },
                copy_data: Default::default(),
                pixels: Default::default(),
            });
        }
        (
            Self {
                shared_mem,
                receiver,
                width,
                height,
            },
            sender_capability,
        )
    }

    pub fn addr(&self) -> usize {
        self.shared_mem.addr().into()
    }

    pub fn size(&self) -> usize {
        let pixel_count = (self.width * self.height) as usize;
        (size_of::<WindowSharedMem>() + size_of::<u32>() * pixel_count)
            .next_multiple_of(AllocPageSize::_2MiB.size_bytes() as usize)
    }

    pub fn channel_id(&self) -> NonZero<u64> {
        self.receiver.channel_id()
    }

    /// Copy data is not validated. You must validate it yourself.
    pub fn copy_to_frame_buffer(
        &mut self,
        copy_data: CopyData,
        frame_buffer: &mut FrameBufferEmbeddedGraphics<'_>,
        x: u64,
        y: u64,
    ) {
        // log::debug!("S: {self:X?}, copy_data: {copy_data:?}. x: {x}. y: {y}");
        let mut shared_mem_ptr = self.shared_mem.cast::<WindowSharedMem>();
        let shared_mem = unsafe { shared_mem_ptr.as_mut() };
        shared_mem.pending_request.store(false, Ordering::Relaxed);
        let pixels_ptr = shared_mem.pixels.as_mut_ptr();
        let pixel_count = (self.width * self.height) as usize;
        let pixels = unsafe { slice::from_raw_parts_mut(pixels_ptr, pixel_count) };
        for src_y in copy_data.y as usize..(copy_data.y + copy_data.height) as usize {
            let src_start_index = self.width as usize * src_y + copy_data.x as usize;
            let dest_start_index = frame_buffer.info().pitch as usize / size_of::<u32>()
                * (y as usize + src_y)
                + x as usize
                + copy_data.x as usize;
            let copy_len = copy_data.width as usize;
            frame_buffer.buffer_mut()[dest_start_index..dest_start_index + copy_len]
                .copy_from_slice(&pixels[src_start_index..src_start_index + copy_len]);
        }
    }

    /// # Cancel safety
    /// This is cancel safe.
    pub async fn handle_draw_request(
        &mut self,
        executor_context: &ExecutorContext,
        frame_buffer: &mut FrameBufferEmbeddedGraphics<'_>,
        x: u64,
        y: u64,
    ) {
        let mut shared_mem_ptr = self.shared_mem.cast::<WindowSharedMem>();
        let shared_mem = unsafe { shared_mem_ptr.as_mut() };
        loop {
            if shared_mem.pending_request.swap(false, Ordering::Acquire) {
                break;
            }
            // This is cancel safe because we didn't modify anything before this
            self.receiver.receive(executor_context).await;
        }
        let copy_data = shared_mem.copy_data;
        if copy_data.x + copy_data.width <= self.width
            && copy_data.y + copy_data.height <= self.height
        {
            self.copy_to_frame_buffer(shared_mem.copy_data, frame_buffer, x, y);
        }
    }
}
