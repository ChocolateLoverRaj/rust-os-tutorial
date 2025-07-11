use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use atomic_enum::atomic_enum;

#[atomic_enum]
pub enum KeyboardResponseResult {
    Ok,
    Err,
}

#[repr(C)]
pub struct KeyboardSharedMem {
    /// 0 - Not requested
    /// >0 - Buffer size
    pub keyboard_request: AtomicUsize,
    /// Channel tx to notify that the keyboard is requested
    pub keyboard_request_tx: u64,
    /// Server sets this after the keyboard is requested
    pub keyboard_response_result: AtomicKeyboardResponseResult,
    /// The server sets this to the address of the keyboard buffer shared mem, if successfull.
    /// Depending on the buffer size, this same page could be reused.
    pub keyboard_response: AtomicUsize,
    /// Channel rx to notify that the keyboard response is available (result and buffer address if set)
    pub keyboard_response_rx: u64,
}

/// A single trusted producer, single non-trusted consumer lock-free u8 queue.
/// Pre-allocated slots without dynamic buffer growth.
#[derive(Debug)]
#[repr(C)]
pub struct ConcurrentQueue {
    write_count: AtomicUsize,
    read_count: AtomicUsize,
    buffer: [AtomicU8; 100],
}

impl Default for ConcurrentQueue {
    fn default() -> Self {
        ConcurrentQueue {
            write_count: AtomicUsize::new(0),
            read_count: AtomicUsize::new(0),
            buffer: core::array::from_fn(|_| Default::default()),
        }
    }
}

/// A writer does not trust the reader
pub struct Writer<'a> {
    queue: &'a ConcurrentQueue,
    write_count: usize,
}

impl<'a> Writer<'a> {
    /// Creating multiple writers will cause unexpected behavior
    pub fn new(queue: &'a ConcurrentQueue) -> Self {
        Self {
            queue,
            write_count: 0,
        }
    }
}

#[derive(Debug)]
pub enum PushError {
    /// For some reason the read count is more than the write count
    InvalidState,
    /// The buffer is full
    NoSlotsAvailable(u8),
}

impl Writer<'_> {
    pub fn push(&mut self, item: u8) -> Result<(), PushError> {
        let read_count = self.queue.read_count.load(Ordering::Relaxed);
        let slots_available = 100
            - self
                .write_count
                .checked_sub(read_count)
                .ok_or(PushError::InvalidState)?;
        if slots_available >= 1 {
            let slot_index = self.write_count % 100;
            self.queue.buffer[slot_index].store(item, Ordering::Relaxed);
            self.write_count += 1;
            self.queue
                .write_count
                .store(self.write_count, Ordering::Release);
        } else {
            Err(PushError::NoSlotsAvailable(item))?;
        }
        Ok(())
    }
}

/// A reader trusts the writer
pub struct Reader<'a> {
    queue: &'a ConcurrentQueue,
}

impl<'a> Reader<'a> {
    /// Creating multiple readers will cause unexpected behavior
    pub fn new(queue: &'a ConcurrentQueue) -> Self {
        Self { queue }
    }
}

impl Reader<'_> {
    pub fn pop(&mut self) -> Option<u8> {
        let read_count = self.queue.read_count.load(Ordering::Relaxed);
        let write_count = self.queue.write_count.load(Ordering::Relaxed);
        if write_count > read_count {
            // This makes the data in slots read what the writer wrote, and not read something outdated
            core::sync::atomic::fence(Ordering::Acquire);
            let slot_index = read_count % 100;
            let value = self.queue.buffer[slot_index].load(Ordering::Relaxed);
            self.queue
                .read_count
                .store(read_count + 1, Ordering::Relaxed);
            Some(value)
        } else {
            None
        }
    }
}
