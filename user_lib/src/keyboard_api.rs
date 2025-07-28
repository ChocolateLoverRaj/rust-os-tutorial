use core::{
    mem::MaybeUninit,
    num::{NonZero, NonZeroUsize},
    ptr::NonNull,
    slice,
    sync::atomic::{AtomicU8, AtomicUsize, Ordering},
};

use atomic_enum::atomic_enum;
use common::{
    AllocPageSize, SyscallAllocError, SyscallNewSharedMem, SyscallNewSharedMemInput, log,
};
use zerocopy::{FromBytes, IntoBytes, KnownLayout};

use crate::{
    EnvEntries, ExecutorContext,
    async_channel::{self, Receiver, Sender},
    syscall, syscall_alloc,
};

pub const KEYBOARD_ENV_KEY: u64 = 0x1D099897F69410FA;

#[atomic_enum]
pub enum KeyboardResponseResult {
    None,
    Ok,
    Err,
}

impl KeyboardResponseResult {
    pub fn result(self) -> Option<Result<(), ()>> {
        match self {
            KeyboardResponseResult::None => None,
            KeyboardResponseResult::Ok => Some(Ok(())),
            KeyboardResponseResult::Err => Some(Err(())),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct KeyboardSharedMem {
    /// 0 - Not requested
    /// >0 - Buffer size
    ///
    /// The server sets this back to 0 when processing the request
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

pub struct KeyboardSharedMemServer {
    shared_mem: NonNull<KeyboardSharedMem>,
    unused_allocated_bytes: NonNull<[u8]>,
    keyboard_request_rx: Receiver,
    keyboard_response_tx: Sender,
}

impl KeyboardSharedMemServer {
    pub fn new() -> Result<Self, SyscallAllocError> {
        let used_len = size_of::<Self>();
        let page_size = AllocPageSize::_4KiB;
        let mut allocated_bytes = syscall_alloc(
            (used_len as u64)
                .next_multiple_of(page_size.size_bytes())
                .try_into()
                .unwrap(),
            page_size,
        )?;
        let allocated_bytes = unsafe { allocated_bytes.as_mut() };
        let (keyboard_shared_mem, unused_allocated_bytes) =
            allocated_bytes.split_at_mut(size_of::<KeyboardSharedMem>());
        let (keyboard_request_tx, keyboard_request_rx) = async_channel::create();
        let (keyboard_response_tx, keyboard_response_rx) = async_channel::create();
        let shared_mem_ptr =
            (keyboard_shared_mem as *mut [u8]).cast::<MaybeUninit<KeyboardSharedMem>>();
        let shared_mem = unsafe { shared_mem_ptr.as_mut() }
            .unwrap()
            .write(KeyboardSharedMem {
                keyboard_request: AtomicUsize::new(0),
                keyboard_request_tx: keyboard_request_tx.channel_id(),
                keyboard_response_result: AtomicKeyboardResponseResult::new(
                    KeyboardResponseResult::None,
                ),
                keyboard_response: AtomicUsize::new(0),
                keyboard_response_rx: keyboard_response_rx.channel_id(),
            });
        Ok(Self {
            keyboard_request_rx,
            keyboard_response_tx,
            shared_mem: NonNull::from_mut(shared_mem),
            unused_allocated_bytes: NonNull::from_mut(unused_allocated_bytes),
        })
    }

    pub fn share_addr(&self) -> usize {
        self.shared_mem.addr().into()
    }

    pub fn share_len(&self) -> usize {
        size_of::<KeyboardSharedMem>() + self.unused_allocated_bytes.len()
    }

    pub fn request_channel_id(&self) -> u64 {
        self.keyboard_request_rx.channel_id()
    }

    pub fn response_channel_id(&self) -> u64 {
        self.keyboard_response_tx.channel_id()
    }

    pub async fn wait_for_request(&mut self, executor_context: &ExecutorContext) -> NonZero<usize> {
        let shared_mem = unsafe { self.shared_mem.as_mut() };
        let requested_buffer_len = loop {
            if let Some(requested_buffer_len) =
                NonZero::new(shared_mem.keyboard_request.swap(0, Ordering::Relaxed))
            {
                break requested_buffer_len;
            }
            self.keyboard_request_rx.receive(executor_context).await;
        };
        requested_buffer_len
    }
}

impl Drop for KeyboardSharedMemServer {
    fn drop(&mut self) {
        // TODO: Unmap shared mem
    }
}

pub struct KeyboardSharedMemClient {
    shared_mem: &'static mut KeyboardSharedMem,
    request_tx: Sender,
    response_rx: Receiver,
}

impl KeyboardSharedMemClient {
    /// # Safety
    /// Only have 1 client
    pub unsafe fn new(env: &EnvEntries) -> Option<Self> {
        let addr = *env.get(&KEYBOARD_ENV_KEY)? as *mut KeyboardSharedMem;
        let shared_mem = unsafe { addr.as_mut() }.unwrap();
        Some(Self {
            request_tx: unsafe { Sender::from_channel_id(shared_mem.keyboard_request_tx) },
            response_rx: unsafe { Receiver::from_channel_id(shared_mem.keyboard_response_rx) },
            shared_mem,
        })
    }

    pub async fn request(
        &mut self,
        executor_context: &ExecutorContext,
        slots_len: NonZeroUsize,
    ) -> Result<(), ()> {
        self.shared_mem
            .keyboard_request
            .store(slots_len.into(), Ordering::Relaxed);
        self.request_tx.send();
        loop {
            if let Some(result) = self
                .shared_mem
                .keyboard_response_result
                .load(Ordering::Acquire)
                .result()
            {
                break result;
            };
            self.response_rx.receive(executor_context).await;
        }?;
        let addr = self.shared_mem.keyboard_response.load(Ordering::Relaxed);
        log::info!("response addr: {addr:X}");
        Ok(())
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct KeyboardBufSharedMem {
    write_count: AtomicUsize,
    new_data_channel: u64,
    read_count: AtomicUsize,
    slots_len: usize,
    slots: [AtomicU8; 0],
}

pub struct KeyboardBufServer {
    shared_mem: NonNull<[u8]>,
    slots_len: usize,
    new_data_sender: Sender,
}

impl KeyboardBufServer {
    pub fn new(slots_len: usize) -> Result<Self, SyscallAllocError> {
        let len = size_of::<KeyboardBufSharedMem>() + size_of::<AtomicU8>() * slots_len;
        let len_2 = NonZero::new(len.try_into().unwrap()).unwrap();

        let input = SyscallNewSharedMemInput {
            page_size: AllocPageSize::_4KiB,
            pages_len: len.div_ceil(AllocPageSize::_4KiB.len()),
        };
        let capability = unsafe { syscall::<SyscallNewSharedMem>(&input) }.unwrap();
        log::debug!("KeyboardBufServer::new: capability = {:?}", capability);

        let shared_mem = syscall_alloc(len_2, AllocPageSize::_4KiB)?;
        let mut ptr = shared_mem.cast::<MaybeUninit<KeyboardBufSharedMem>>();
        let (sender, receiver) = async_channel::create();
        unsafe { ptr.as_mut() }.write(KeyboardBufSharedMem {
            write_count: Default::default(),
            new_data_channel: receiver.channel_id(),
            read_count: Default::default(),
            slots_len,
            slots: Default::default(),
        });
        Ok(Self {
            shared_mem,
            slots_len,
            new_data_sender: sender,
        })
    }
}

#[derive(Debug, FromBytes, IntoBytes, KnownLayout)]
#[repr(C)]
pub struct SharedConcurrentQueue {
    write_count: AtomicUsize,
    read_count: AtomicUsize,
    slots_count: usize,
    /// A dynamic number of items
    slots: [AtomicU8; 0],
}

impl SharedConcurrentQueue {
    pub fn len(slots_len: usize) -> usize {
        size_of::<Self>() + size_of::<AtomicU8>() * slots_len
    }

    /// Size of slice must be exact. Bytes can be uninitialized. They will be zeroed by this function.
    pub fn init(bytes: &mut [u8]) -> &mut Self {
        let (queue_bytes, slots_bytes) = bytes.split_at_mut(size_of::<Self>());
        let s = Self::mut_from_bytes(queue_bytes).unwrap();
        *s = Self {
            write_count: AtomicUsize::new(0),
            read_count: AtomicUsize::new(0),
            slots_count: slots_bytes.len(),
            slots: [],
        };
        slots_bytes.fill(0);
        s
    }

    /// Do not have multiple writers at the same time
    ///
    /// # Safety
    /// The slots len should be correct
    pub unsafe fn get_writer(&self, slots_len: usize) -> QueueWriter<'_> {
        let slots = (&self.slots as *const [AtomicU8; 0]).cast::<AtomicU8>();
        let slots = unsafe { slice::from_raw_parts(slots, slots_len) };
        QueueWriter::new(&self.write_count, &self.read_count, slots)
    }

    /// Do not have multiple readers at the same time
    ///
    /// # Safety
    /// The slots len should be correct
    pub unsafe fn get_reader(&self, slots_len: usize) -> QueueReader<'_> {
        let slots = (&self.slots as *const [AtomicU8; 0]).cast::<AtomicU8>();
        let slots = unsafe { slice::from_raw_parts(slots, slots_len) };
        QueueReader::new(&self.write_count, &self.read_count, slots)
    }
}

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
            let slot_index = read_count % 100;
            let value = self.shared_slots[slot_index].load(Ordering::Relaxed);
            self.shared_read_count
                .store(read_count + 1, Ordering::Relaxed);
            Some(value)
        } else {
            None
        }
    }
}
