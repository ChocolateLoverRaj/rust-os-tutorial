use embedded_graphics::{pixelcolor::Rgb888, prelude::RgbColor};

#[derive(Debug, Clone, Copy)]
pub struct RgbPixelInfo {
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
}

impl RgbPixelInfo {
    /// Technically, Limine and this struct could have a pixel size other than u32, in which case you shouldn't use this method
    pub fn build_pixel(&self, color: Rgb888) -> u32 {
        let mut n = 0;
        n |= ((color.r() as u32) & ((1 << self.red_mask_size) - 1)) << self.red_mask_shift;
        n |= ((color.g() as u32) & ((1 << self.green_mask_size) - 1)) << self.green_mask_shift;
        n |= ((color.b() as u32) & ((1 << self.blue_mask_size) - 1)) << self.blue_mask_shift;
        n
    }
}
