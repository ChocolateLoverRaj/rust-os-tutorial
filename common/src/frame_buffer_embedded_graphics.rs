use core::convert::Infallible;

pub use embedded_graphics;
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, DrawTarget, Point, RgbColor, Size},
    primitives::Rectangle,
};

use crate::FrameBufferInfo;

pub struct FrameBufferEmbeddedGraphics<'a> {
    buffer: &'a mut [u8],
    info: FrameBufferInfo,
}

impl<'a> FrameBufferEmbeddedGraphics<'a> {
    /// # Safety
    /// The frame buffer must be mapped at `addr`
    pub unsafe fn new(addr: *mut u8, info: FrameBufferInfo) -> Self {
        if info.bits_per_pixel == 8 * 4 {
            Self {
                buffer: {
                    let len = (info.pitch * info.height) as usize;
                    // Safety: This memory is mapped
                    unsafe { core::slice::from_raw_parts_mut(addr, len) }
                },
                info,
            }
        } else {
            panic!("DrawTarget implemented for RGB888, but bpp doesn't match RGB888");
        }
    }

    fn get_pixel(&self, color: Rgb888) -> [u8; 4] {
        let mut n = 0;
        n |=
            ((color.r() as u32) & ((1 << self.info.red_mask_size) - 1)) << self.info.red_mask_shift;
        n |= ((color.g() as u32) & ((1 << self.info.green_mask_size) - 1))
            << self.info.green_mask_shift;
        n |= ((color.b() as u32) & ((1 << self.info.blue_mask_size) - 1))
            << self.info.blue_mask_shift;
        n.to_ne_bytes()
    }

    /// Moves everything on the screen up, leaving the bottom the same as it was before
    pub fn shift_up(&mut self, amount: u32) {
        let pitch = self.info.pitch;
        self.buffer
            .copy_within(amount as usize * pitch as usize..self.buffer.len(), 0);
    }
}

impl DrawTarget for FrameBufferEmbeddedGraphics<'_> {
    type Color = Rgb888;

    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        let bytes_per_pixel = (self.info.bits_per_pixel / 8) as usize;
        pixels.into_iter().for_each(|pixel| {
            let point = pixel.0;
            if (0..self.info.width).contains(&(point.x as u64))
                && (0..self.info.height).contains(&(point.y as u64))
            {
                let color = pixel.1;
                let buffer_position = point.y as usize * self.info.pitch as usize
                    + point.x as usize * bytes_per_pixel;
                let pixel = self.get_pixel(color);
                self.buffer[buffer_position..buffer_position + bytes_per_pixel]
                    .copy_from_slice(&pixel);
            }
        });
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let pixel = self.get_pixel(color);
        let bytes_per_pixel = (self.info.bits_per_pixel / 8) as usize;
        let pitch = self.info.pitch as usize;
        // Draw to the top row
        for x in area.top_left.x..area.top_left.x + area.size.width as i32 {
            let buffer_position = area.top_left.y as usize * pitch + x as usize * bytes_per_pixel;
            self.buffer[buffer_position..buffer_position + bytes_per_pixel].copy_from_slice(&pixel);
        }
        // Copy the top row to all other rows
        let top_row_start =
            area.top_left.y as usize * pitch + area.top_left.x as usize * bytes_per_pixel;
        let top_row = top_row_start..top_row_start + area.size.width as usize * bytes_per_pixel;
        for y in area.top_left.y + 1..area.top_left.y + area.size.height as i32 {
            let row_start = y as usize * pitch + area.top_left.x as usize * bytes_per_pixel;
            self.buffer.copy_within(top_row.clone(), row_start);
        }
        Ok(())
    }
}

impl Dimensions for FrameBufferEmbeddedGraphics<'_> {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        Rectangle {
            top_left: Point { x: 0, y: 0 },
            size: Size {
                width: self.info.width.try_into().unwrap(),
                height: self.info.height.try_into().unwrap(),
            },
        }
    }
}
