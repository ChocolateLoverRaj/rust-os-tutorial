use core::fmt::Write;

use unicode_segmentation::UnicodeSegmentation;

/// A writer that writes to a writer but replaces `\n` with `\r\n`
pub struct WriterWithCr<T> {
    writer: T,
}

impl<T> WriterWithCr<T> {
    pub const fn new(writer: T) -> Self {
        Self { writer }
    }
}

impl<T: Write> Write for WriterWithCr<T> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.graphemes(true) {
            match c {
                "\n" => self.writer.write_str("\r\n")?,
                s => self.writer.write_str(s)?,
            }
        }
        Ok(())
    }
}
