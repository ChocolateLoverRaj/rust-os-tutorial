use core::ops::{Deref, DerefMut};

use common::{FrameBufferEmbeddedGraphics, SyscallTakeFrameBufferError};

use crate::syscalls::{syscall_release_frame_buffer, syscall_take_frame_buffer};

/// A Rust safe wrapper that releases the frame buffer when dropped
pub struct FrameBuffer {
    frame_buffer_embedded_graphics: FrameBufferEmbeddedGraphics<'static>,
}

impl FrameBuffer {
    pub fn try_new() -> Result<Self, SyscallTakeFrameBufferError> {
        let output = syscall_take_frame_buffer()?;
        let ptr = output.ptr as *mut u8;
        // Safety: the kernel mapped the frame buffer
        let frame_buffer_embedded_graphics =
            unsafe { FrameBufferEmbeddedGraphics::new(ptr, output.info) };
        Ok(Self {
            frame_buffer_embedded_graphics,
        })
    }
}

impl Drop for FrameBuffer {
    fn drop(&mut self) {
        syscall_release_frame_buffer();
    }
}

impl Deref for FrameBuffer {
    type Target = FrameBufferEmbeddedGraphics<'static>;

    fn deref(&self) -> &Self::Target {
        &self.frame_buffer_embedded_graphics
    }
}

impl DerefMut for FrameBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.frame_buffer_embedded_graphics
    }
}
