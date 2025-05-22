use core::fmt::{Display, Write};

use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyleBuilder, iso_8859_16::FONT_10X20},
    pixelcolor::Rgb888,
    prelude::{Dimensions, DrawTarget, Point, Primitive, RgbColor, Size},
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::{Baseline, Text},
};
use limine::response::FramebufferResponse;
use log::{Level, LevelFilter, Log};
use owo_colors::OwoColorize;
use uart_16550::SerialPort;
use unicode_segmentation::UnicodeSegmentation;

use crate::frame_buffer_embedded_graphics::FrameBufferEmbeddedGraphics;

struct DisplayData {
    display: FrameBufferEmbeddedGraphics<'static>,
    position: Point,
}

struct Inner {
    serial_port: SerialPort,
    display: Option<DisplayData>,
}

/// Represents a color in a terminal or screen. The default color may depend on if the theme is light or dark.
enum Color {
    Default,
    BrightRed,
    BrightYellow,
    BrightBlue,
    BrightCyan,
    BrightMagenta,
}

impl Inner {
    fn write_with_color(&mut self, color: Color, string: impl Display) {
        // Write to serial
        {
            let string: &dyn Display = match color {
                Color::Default => &string,
                Color::BrightRed => &string.bright_red(),
                Color::BrightYellow => &string.bright_yellow(),
                Color::BrightBlue => &string.bright_blue(),
                Color::BrightCyan => &string.bright_cyan(),
                Color::BrightMagenta => &string.bright_magenta(),
            };
            write!(self.serial_port, "{string}").unwrap();
        }

        // Write to screen
        if let Some(display_data) = &mut self.display {
            struct Writer<'a> {
                display: &'a mut FrameBufferEmbeddedGraphics<'static>,
                position: &'a mut Point,
                text_color: <FrameBufferEmbeddedGraphics<'a> as DrawTarget>::Color,
            }
            impl Write for Writer<'_> {
                fn write_str(&mut self, s: &str) -> core::fmt::Result {
                    let font = FONT_10X20;
                    let background_color = Rgb888::BLACK;
                    for c in s.graphemes(true) {
                        let height_not_seen = self.position.y + font.character_size.height as i32
                            - self.display.bounding_box().size.height as i32;
                        if height_not_seen > 0 {
                            self.display.shift_up(height_not_seen as u32);
                            self.position.y -= height_not_seen;
                        }
                        match c {
                            "\r" => {
                                // We do not handle special cursor movements
                            }
                            "\n" | "\r\n" => {
                                // Fill the remaining space with background color
                                Rectangle::new(
                                    *self.position,
                                    Size::new(
                                        self.display.bounding_box().size.width
                                            - self.position.x as u32,
                                        font.character_size.height,
                                    ),
                                )
                                .into_styled(
                                    PrimitiveStyleBuilder::new()
                                        .fill_color(background_color)
                                        .build(),
                                )
                                .draw(self.display)
                                .map_err(|_| core::fmt::Error)?;
                                self.position.y += font.character_size.height as i32;
                                self.position.x = 0;
                            }
                            c => {
                                let style = MonoTextStyleBuilder::new()
                                    .font(&font)
                                    .text_color(self.text_color)
                                    .background_color(background_color)
                                    .build();
                                *self.position =
                                    Text::with_baseline(c, *self.position, style, Baseline::Top)
                                        .draw(self.display)
                                        .map_err(|_| core::fmt::Error)?;
                                if self.position.x as u32 + font.character_size.width
                                    > self.display.bounding_box().size.width
                                {
                                    self.position.y += font.character_size.height as i32;
                                    self.position.x = 0;
                                }
                            }
                        }
                    }
                    Ok(())
                }
            }
            let mut writer = Writer {
                display: &mut display_data.display,
                position: &mut display_data.position,
                text_color: match color {
                    Color::Default => Rgb888::WHITE,
                    // Mimick the ANSI escape colors
                    Color::BrightRed => Rgb888::new(255, 85, 85),
                    Color::BrightYellow => Rgb888::new(255, 255, 85),
                    Color::BrightBlue => Rgb888::new(85, 85, 255),
                    Color::BrightCyan => Rgb888::new(85, 255, 255),
                    Color::BrightMagenta => Rgb888::new(255, 85, 255),
                },
            };
            write!(writer, "{string}").unwrap();
        }
    }
}

struct KernelLogger {
    inner: spin::Mutex<Inner>,
}

impl Log for KernelLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        todo!()
    }

    fn log(&self, record: &log::Record) {
        let mut inner = self.inner.lock();
        let level = record.level();
        inner.write_with_color(
            match level {
                Level::Error => Color::BrightRed,
                Level::Warn => Color::BrightYellow,
                Level::Info => Color::BrightBlue,
                Level::Debug => Color::BrightCyan,
                Level::Trace => Color::BrightMagenta,
            },
            format_args!("{level:5} "),
        );
        inner.write_with_color(Color::Default, record.args());
        inner.write_with_color(Color::Default, "\n");
    }

    fn flush(&self) {
        todo!()
    }
}

static LOGGER: KernelLogger = KernelLogger {
    inner: spin::Mutex::new(Inner {
        serial_port: unsafe { SerialPort::new(0x3F8) },
        display: None,
    }),
};

pub fn init(frame_buffer: &'static FramebufferResponse) -> Result<(), log::SetLoggerError> {
    let mut inner = LOGGER.inner.try_lock().unwrap();
    inner.serial_port.init();
    inner.display = frame_buffer
        .framebuffers()
        .next()
        .map(|frame_buffer| DisplayData {
            display: FrameBufferEmbeddedGraphics::new(frame_buffer),
            position: Point::zero(),
        });
    log::set_max_level(LevelFilter::max());
    log::set_logger(&LOGGER)
}
