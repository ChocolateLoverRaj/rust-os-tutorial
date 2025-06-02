use core::{array::TryFromSliceError, marker::PhantomData};

use elf::{ElfBytes, endian::EndianParse};
use nano_leb128::{LEB128DecodeError, ULEB128};
use thiserror::Error;

#[derive(Debug)]
pub struct StackSizeItem {
    pub symbol: u64,
    pub stack_size: u64,
}

pub struct StackSizesIterator<'data, E: EndianParse> {
    data: &'data [u8],
    position: usize,
    endian_parse: PhantomData<E>,
}

#[derive(Debug, Error)]
pub enum ParseFromElfError {
    #[error("Error originating from the elf crate")]
    ElfParseError(elf::ParseError),
    #[error("Section data is compressed. We currently don't handle this")]
    IsCompressed,
}
impl<'data, E: EndianParse> StackSizesIterator<'data, E> {
    /// Returns `None` if the `.stack_sizes` section is not present
    pub fn try_parse_from_elf(
        elf: &'data ElfBytes<'data, E>,
    ) -> Result<Option<Self>, ParseFromElfError> {
        Ok({
            if let Some(stack_sizes) = elf
                .section_header_by_name(".stack_sizes")
                .map_err(ParseFromElfError::ElfParseError)?
            {
                let (data, compression_header) = elf
                    .section_data(&stack_sizes)
                    .map_err(ParseFromElfError::ElfParseError)?;
                if compression_header.is_some() {
                    Err(ParseFromElfError::IsCompressed)?;
                }
                Some(Self {
                    data,
                    position: 0,
                    endian_parse: PhantomData,
                })
            } else {
                None
            }
        })
    }
}

#[derive(Debug, Error)]
pub enum ParseItemError {
    #[error("Unexpected end of slice when parsing")]
    ErrorParsingSymbol(TryFromSliceError),
    #[error("Error when reading the unsigned LEB128")]
    ErrorParsingStackSize(LEB128DecodeError),
}
impl<E: EndianParse> Iterator for StackSizesIterator<'_, E> {
    type Item = Result<StackSizeItem, ParseItemError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.position < self.data.len() {
            Some((|| {
                let symbol = u64::from_ne_bytes(
                    self.data[self.position..self.position + size_of::<u64>()]
                        .try_into()
                        .map_err(ParseItemError::ErrorParsingSymbol)?,
                );
                self.position += size_of::<u64>();
                let (stack_size, len) = ULEB128::read_from(&self.data[self.position..])
                    .map_err(ParseItemError::ErrorParsingStackSize)?;
                self.position += len;
                Ok(StackSizeItem {
                    symbol: symbol.into(),
                    stack_size: stack_size.into(),
                })
            })())
        } else {
            None
        }
    }
}
