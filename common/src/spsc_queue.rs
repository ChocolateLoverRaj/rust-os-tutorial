use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

/// A writer does not trust the reader
pub struct QueueWriter<'a> {
    shared_write_count: &'a AtomicUsize,
    shared_read_count: &'a AtomicUsize,
    shared_slots: &'a [AtomicU8],
}

impl<'a> QueueWriter<'a> {
    /// Creating multiple writers will cause unexpected behavior
    pub fn new(
        shared_write_count: &'a AtomicUsize,
        shared_read_count: &'a AtomicUsize,
        slots: &'a [AtomicU8],
    ) -> Self {
        Self {
            shared_write_count,
            shared_read_count,
            shared_slots: slots,
        }
    }
}

#[derive(Debug)]
pub enum PushError {
    /// For some reason the read count or write count is an invalid value
    InvalidState,
    /// The buffer is full
    NoSlotsAvailable(u8),
    /// The write count overflowed. This should not happen, but it could happen if the reader messes up the write count.
    WriteCountOverflow,
}

impl QueueWriter<'_> {
    pub fn push(&mut self, item: u8) -> Result<(), PushError> {
        let read_count = self.shared_read_count.load(Ordering::Relaxed);
        let write_count = self.shared_write_count.load(Ordering::Relaxed);
        if self.shared_slots.len()
            > write_count
                .checked_sub(read_count)
                .ok_or(PushError::InvalidState)?
        {
            let slot_index = write_count % self.shared_slots.len();
            self.shared_slots[slot_index].store(item, Ordering::Relaxed);
            self.shared_write_count.store(
                write_count
                    .checked_add(1)
                    .ok_or(PushError::WriteCountOverflow)?,
                Ordering::Release,
            );
        } else {
            Err(PushError::NoSlotsAvailable(item))?;
        }
        Ok(())
    }
}

/// A reader trusts the writer
pub struct QueueReader<'a> {
    shared_write_count: &'a AtomicUsize,
    shared_read_count: &'a AtomicUsize,
    shared_slots: &'a [AtomicU8],
}

impl<'a> QueueReader<'a> {
    /// Creating multiple readers will cause unexpected behavior
    pub fn new(
        shared_write_count: &'a AtomicUsize,
        shared_read_count: &'a AtomicUsize,
        shared_slots: &'a [AtomicU8],
    ) -> Self {
        Self {
            shared_write_count,
            shared_read_count,
            shared_slots,
        }
    }
}

impl QueueReader<'_> {
    pub fn pop(&mut self) -> Option<u8> {
        let read_count = self.shared_read_count.load(Ordering::Relaxed);
        let write_count = self.shared_write_count.load(Ordering::Relaxed);
        if write_count > read_count {
            // This makes the data in slots read what the writer wrote, and not read something outdated
            core::sync::atomic::fence(Ordering::Acquire);
            let slot_index = read_count % self.shared_slots.len();
            let value = self.shared_slots[slot_index].load(Ordering::Relaxed);
            self.shared_read_count
                .store(read_count + 1, Ordering::Relaxed);
            Some(value)
        } else {
            None
        }
    }
}
