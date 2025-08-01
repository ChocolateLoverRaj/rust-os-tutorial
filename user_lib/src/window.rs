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
    AllocPageSize, FrameBufferEmbeddedGraphics, PermissionFlags, RgbPixelInfo,
    SyscallMapSharedMemError, SyscallNewShardMemError, SyscallNewSharedMemInput,
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
    syscall_clone_capability, syscall_map_shared_mem, syscall_new_shared_mem,
};

pub const ENV_KEY: u64 = 0x2994A6830F66D288;

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
pub struct WindowSharedMemClient {
    shared_mem: NonNull<[u8]>,
}

impl WindowSharedMemClient {
    /// # Safety
    /// This function should only be called once, because calling this function multiple times will result in multiple simultaneous mutable references.
    pub unsafe fn new(env_entries: &EnvEntries) -> Result<Self, SyscallMapSharedMemError> {
        let capability = NonZero::new(*env_entries.get(&ENV_KEY).unwrap()).unwrap();
        let shared_mem = syscall_map_shared_mem(
            capability,
            PermissionFlags::READABLE | PermissionFlags::WRITABLE,
        )?;
        Ok(Self { shared_mem })
    }

    fn mem(&self) -> &WindowSharedMem {
        let ptr = self.shared_mem.cast::<WindowSharedMem>();
        unsafe { ptr.as_ref() }
    }

    fn mem_mut(&mut self) -> &mut WindowSharedMem {
        let mut ptr = self.shared_mem.cast::<WindowSharedMem>();
        unsafe { ptr.as_mut() }
    }

    fn buffer(&mut self) -> &mut [u32] {
        let len = (self.mem_mut().window_info.width * self.mem_mut().window_info.height) as usize;
        let ptr = self.mem_mut().pixels.as_mut_ptr();
        unsafe { slice::from_raw_parts_mut(ptr, len) }
    }

    pub fn update_screen(&mut self, copy_data: CopyData) {
        self.mem_mut().copy_data = copy_data;
        let mut sender = unsafe { Sender::from_channel_id(self.mem_mut().request_channel_id) };
        if !self.mem_mut().pending_request.swap(true, Ordering::Release) {
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
        let width = self.mem_mut().window_info.width;
        let pixel_info = self.mem_mut().window_info.pixel_info;
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
                self.mem().window_info.width.try_into().unwrap(),
                self.mem().window_info.height.try_into().unwrap(),
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

#[derive(Debug)]
pub enum NewWindowServerError {
    NewSharedMem(SyscallNewShardMemError),
    MapSharedMem(SyscallMapSharedMemError),
}

impl WindowSharedMemServer {
    pub fn new(
        width: u64,
        height: u64,
        frame_buffer: &FrameBufferEmbeddedGraphics,
    ) -> Result<(Self, NonZero<u64>, NonZero<u64>), NewWindowServerError> {
        let pixel_count = (width * height) as usize;
        let used_len = size_of::<WindowSharedMem>() + size_of::<u32>() * pixel_count;
        let page_size = AllocPageSize::_2MiB;
        let shared_mem_capability = syscall_new_shared_mem(SyscallNewSharedMemInput {
            page_size,
            pages_len: used_len.div_ceil(page_size.byte_len()),
        })
        .map_err(NewWindowServerError::NewSharedMem)?;
        let client_shared_mem_capability = syscall_clone_capability(shared_mem_capability).unwrap();
        let shared_mem = syscall_map_shared_mem(
            shared_mem_capability,
            PermissionFlags::READABLE | PermissionFlags::WRITABLE,
        )
        .map_err(NewWindowServerError::MapSharedMem)?;
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
        Ok((
            Self {
                shared_mem,
                receiver,
                width,
                height,
            },
            client_shared_mem_capability,
            sender_capability,
        ))
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
