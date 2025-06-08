use core::{
    fmt::{Display, Write},
    ops::{Deref, DerefMut},
};

use alloc::boxed::Box;
use common::FrameBufferEmbeddedGraphics;
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

use crate::{cpu_local_data::try_get_local, writer_with_cr::WriterWithCr};

struct DisplayData {
    display: FrameBufferEmbeddedGraphics<'static>,
    position: Point,
}

pub enum AnyWriter {
    Com1(SerialPort),
    Boxed(Box<dyn Write + Send + Sync>),
}

impl Deref for AnyWriter {
    type Target = dyn Write;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Com1(r) => r,
            Self::Boxed(b) => b.as_ref(),
        }
    }
}

impl DerefMut for AnyWriter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Com1(r) => r,
            Self::Boxed(b) => b.as_mut(),
        }
    }
}

struct Inner {
    serial_port: Option<AnyWriter>,
    display: Option<DisplayData>,
}

/// Represents a color in a terminal or screen. The default color may depend on if the theme is light or dark.
enum Color {
    Default,
    Gray,
    BrightRed,
    BrightYellow,
    BrightBlue,
    BrightCyan,
    BrightMagenta,
    BrightGreen,
}

impl Color {
    fn for_log_level(level: log::Level) -> Self {
        match level {
            Level::Error => Self::BrightRed,
            Level::Warn => Self::BrightYellow,
            Level::Info => Self::BrightBlue,
            Level::Debug => Self::BrightCyan,
            Level::Trace => Self::BrightMagenta,
        }
    }
}

impl Inner {
    fn write_with_color(&mut self, color: Color, string: impl Display) {
        // Write to serial
        if let Some(serial_port) = &mut self.serial_port {
            let string: &dyn Display = match color {
                Color::Default => &string,
                Color::Gray => &string.dimmed(),
                Color::BrightRed => &string.bright_red(),
                Color::BrightYellow => &string.bright_yellow(),
                Color::BrightBlue => &string.bright_blue(),
                Color::BrightCyan => &string.bright_cyan(),
                Color::BrightMagenta => &string.bright_magenta(),
                Color::BrightGreen => &string.bright_green(),
            };
            // Replace \n with \r\n so that it works with tio / screen
            let mut writer = WriterWithCr::new(serial_port.deref_mut());
            write!(writer, "{string}").unwrap();
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
                    Color::Gray => Rgb888::new(128, 128, 128),
                    Color::BrightRed => Rgb888::new(255, 85, 85),
                    Color::BrightYellow => Rgb888::new(255, 255, 85),
                    Color::BrightBlue => Rgb888::new(85, 85, 255),
                    Color::BrightCyan => Rgb888::new(85, 255, 255),
                    Color::BrightMagenta => Rgb888::new(255, 85, 255),
                    Color::BrightGreen => Rgb888::GREEN,
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
        if let Some(cpu_local_data) = try_get_local() {
            let cpu_id = cpu_local_data.cpu.id;
            inner.write_with_color(Color::Gray, format_args!("[CPU {cpu_id}] "));
        } else {
            inner.write_with_color(Color::Gray, "[BSP] ");
        };
        let level = record.level();
        inner.write_with_color(Color::for_log_level(level), format_args!("{level:5} "));
        inner.write_with_color(Color::Default, record.args());
        inner.write_with_color(Color::Default, "\n");
    }

    fn flush(&self) {
        todo!()
    }
}

static LOGGER: KernelLogger = KernelLogger {
    inner: spin::Mutex::new(Inner {
        serial_port: None,
        display: None,
    }),
};

pub fn init() -> Result<(), log::SetLoggerError> {
    let mut inner = LOGGER.inner.try_lock().unwrap();
    inner.serial_port = Some(AnyWriter::Com1({
        // Safety: this is the only code that is accessing COM1
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        serial_port
    }));
    log::set_max_level(LevelFilter::max());
    log::set_logger(&LOGGER)
}

/// Replaces the serial logger, setting it to `None` if specified
pub fn replace_serial_logger(new_serial_logger: Option<AnyWriter>) {
    LOGGER.inner.lock().serial_port = new_serial_logger;
}

/// Log a message which will be prefixed with a "U" indicating it's from user mode.
/// Remember to clean / strip anything you don't want from the message, such as ANSI escape codes or new lines.
pub fn log_for_user_mode(level: log::Level, message: impl Display) {
    let mut inner = LOGGER.inner.lock();
    inner.write_with_color(Color::BrightGreen, "U ");
    inner.write_with_color(Color::for_log_level(level), format_args!("{level:5} "));
    inner.write_with_color(Color::Default, format_args!("{message}\n"));
}

/// Start logging to the frame buffer from now on
pub fn init_frame_buffer(frame_buffer_response: &'static FramebufferResponse) {
    LOGGER.inner.lock().display = frame_buffer_response
        .framebuffers()
        .next()
        .map(|frame_buffer| DisplayData {
            display: {
                // Safety: The frame buffer is mapped by Limine
                unsafe {
                    FrameBufferEmbeddedGraphics::new(frame_buffer.addr(), (&frame_buffer).into())
                }
            },
            position: Point::zero(),
        });
}

/// Stop logging to the frame buffer
pub fn take_frame_buffer() -> Option<()> {
    LOGGER.inner.lock().display.take().map(|_| ())
}
