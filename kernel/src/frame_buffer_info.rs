use crate::RgbPixelInfo;

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct FrameBufferInfo {
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bits_per_pixel: u16,
    pub pixel_info: RgbPixelInfo,
}

impl From<&limine::framebuffer::Framebuffer<'_>> for FrameBufferInfo {
    fn from(framebuffer: &limine::framebuffer::Framebuffer) -> Self {
        FrameBufferInfo {
            width: framebuffer.width(),
            height: framebuffer.height(),
            pitch: framebuffer.pitch(),
            bits_per_pixel: framebuffer.bpp(),
            pixel_info: RgbPixelInfo {
                red_mask_size: framebuffer.red_mask_size(),
                red_mask_shift: framebuffer.red_mask_shift(),
                green_mask_size: framebuffer.green_mask_size(),
                green_mask_shift: framebuffer.green_mask_shift(),
                blue_mask_size: framebuffer.blue_mask_size(),
                blue_mask_shift: framebuffer.blue_mask_shift(),
            },
        }
    }
}
