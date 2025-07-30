use core::{
    mem::MaybeUninit,
    num::NonZero,
    ptr::NonNull,
    slice,
    sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
};

use atomic_enum::atomic_enum;
use common::{
    AllocPageSize, PermissionFlags, PushError, QueueReader, QueueWriter, SyscallAllocError,
    SyscallCloneCapability, SyscallMapSharedMemError, SyscallNewSharedMem,
    SyscallNewSharedMemInput, SyscallSendCapability, SyscallSendCapabilityInput,
};

use crate::{
    EnvEntries, ExecutorContext,
    async_channel::{self, Receiver, Sender},
    syscall, syscall_alloc, syscall_clone_capability, syscall_get_thread_id,
    syscall_map_shared_mem, syscall_send_capability,
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
    pub server_process_id: NonZero<u32>,
    /// 0 => Not requested
    /// _ => Capability of shared mem
    ///
    /// The server sets this back to 0 when processing the request
    pub keyboard_request: AtomicU64,
    /// Channel tx to notify that the keyboard is requested
    pub keyboard_request_tx: NonZero<u64>,
    /// Server sets this after the keyboard is requested
    pub keyboard_response: AtomicBool,
    /// Channel rx to notify that the keyboard response is available (result and buffer address if set)
    pub keyboard_response_rx: NonZero<u64>,
}

pub struct KeyboardSharedMemServer {
    shared_mem: NonNull<KeyboardSharedMem>,
    unused_allocated_bytes: NonNull<[u8]>,
    keyboard_request_rx: Receiver,
    keyboard_response_tx: Sender,
}

impl KeyboardSharedMemServer {
    pub fn new() -> Result<(Self, [NonZero<u64>; 2]), SyscallAllocError> {
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
        let keyboard_request_tx_capability =
            syscall_clone_capability(keyboard_request_tx.channel_id()).unwrap();
        let keyboard_response_rx_capability =
            syscall_clone_capability(keyboard_response_rx.channel_id()).unwrap();
        let shared_mem = unsafe { shared_mem_ptr.as_mut() }
            .unwrap()
            .write(KeyboardSharedMem {
                server_process_id: syscall_get_thread_id().process_id,
                keyboard_request: Default::default(),
                keyboard_request_tx: keyboard_request_tx_capability,
                keyboard_response: Default::default(),
                keyboard_response_rx: keyboard_response_rx_capability,
            });
        Ok((
            Self {
                keyboard_request_rx,
                keyboard_response_tx,
                shared_mem: NonNull::from_mut(shared_mem),
                unused_allocated_bytes: NonNull::from_mut(unused_allocated_bytes),
            },
            [
                keyboard_request_tx_capability,
                keyboard_response_rx_capability,
            ],
        ))
    }

    pub fn share_addr(&self) -> usize {
        self.shared_mem.addr().into()
    }

    pub fn share_len(&self) -> usize {
        size_of::<KeyboardSharedMem>() + self.unused_allocated_bytes.len()
    }

    pub fn request_channel_id(&self) -> NonZero<u64> {
        self.keyboard_request_rx.channel_id()
    }

    pub fn response_channel_id(&self) -> NonZero<u64> {
        self.keyboard_response_tx.channel_id()
    }

    /// # Cancel Safety
    /// The returned future is cancel safe.
    pub async fn wait_for_request(
        &mut self,
        executor_context: &ExecutorContext,
        client_process_id: NonZero<u32>,
    ) -> Result<KeyboardBufServer, SyscallMapSharedMemError> {
        let shared_mem = unsafe { self.shared_mem.as_mut() };
        let shared_mem_capability = loop {
            if let Some(requested_buffer_len) =
                NonZero::new(shared_mem.keyboard_request.swap(0, Ordering::Relaxed))
            {
                break requested_buffer_len;
            }
            // Nothing modified, cancel-safe await
            self.keyboard_request_rx.receive(executor_context).await;
        };
        let buf_shared_mem = syscall_map_shared_mem(
            shared_mem_capability,
            PermissionFlags::READABLE | PermissionFlags::WRITABLE,
        )?;
        let mut ptr = buf_shared_mem.cast::<MaybeUninit<KeyboardBufSharedMem>>();
        let (sender, receiver) = async_channel::create();
        let receiver_capability = syscall_clone_capability(receiver.channel_id()).unwrap();
        syscall_send_capability(SyscallSendCapabilityInput {
            capability: receiver_capability,
            process_id: client_process_id,
        })
        .unwrap();
        unsafe { ptr.as_mut() }.write(KeyboardBufSharedMem {
            write_count: Default::default(),
            new_data_channel: receiver_capability,
            read_count: Default::default(),
            slots: Default::default(),
        });
        shared_mem.keyboard_response.store(true, Ordering::Release);
        self.keyboard_response_tx.send();
        Ok(KeyboardBufServer {
            shared_mem_capability,
            shared_mem: buf_shared_mem,
            new_data_sender: sender,
        })
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
        slots_len: usize,
    ) -> Result<KeyboardBufClient, SyscallMapSharedMemError> {
        let len = size_of::<KeyboardBufSharedMem>() + size_of::<AtomicU8>() * slots_len;
        let input = SyscallNewSharedMemInput {
            page_size: AllocPageSize::_4KiB,
            pages_len: len.div_ceil(AllocPageSize::_4KiB.byte_len()),
        };
        let capability = unsafe { syscall::<SyscallNewSharedMem>(&input) }.unwrap();
        let shared_mem = syscall_map_shared_mem(
            capability,
            PermissionFlags::READABLE | PermissionFlags::WRITABLE,
        )?;
        let server_capability = {
            let server_capability =
                unsafe { syscall::<SyscallCloneCapability>(&capability) }.unwrap();
            let input = SyscallSendCapabilityInput {
                capability: server_capability,
                process_id: self.shared_mem.server_process_id,
            };
            unsafe { syscall::<SyscallSendCapability>(&input) }.unwrap();
            server_capability
        };
        self.shared_mem
            .keyboard_request
            .store(server_capability.get(), Ordering::Relaxed);
        self.request_tx.send();
        loop {
            if self.shared_mem.keyboard_response.load(Ordering::Acquire) {
                break;
            };
            self.response_rx.receive(executor_context).await;
        }
        Ok(KeyboardBufClient {
            shared_mem_capability: capability,
            shared_mem,
        })
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct KeyboardBufSharedMem {
    write_count: AtomicUsize,
    new_data_channel: NonZero<u64>,
    read_count: AtomicUsize,
    /// Whatever shared memory left is for slots
    slots: [AtomicU8; 0],
}

#[derive(Debug)]
pub struct KeyboardBufClient {
    shared_mem_capability: NonZero<u64>,
    shared_mem: NonNull<[u8]>,
}

impl KeyboardBufClient {
    pub async fn read(&mut self, executor_context: &ExecutorContext) -> u8 {
        let slots_len =
            (self.shared_mem.len() - size_of::<KeyboardBufSharedMem>()) / size_of::<AtomicU8>();
        let shared_mem_ptr = self.shared_mem.cast::<KeyboardBufSharedMem>();
        let shared_mem = unsafe { shared_mem_ptr.as_ref() };
        let shared_slots_ptr = shared_mem.slots.as_ptr();
        let shared_slots = unsafe { slice::from_raw_parts(shared_slots_ptr, slots_len) };
        let mut reader = QueueReader::new(
            &shared_mem.write_count,
            &shared_mem.read_count,
            shared_slots,
        );
        let mut receiver = unsafe { Receiver::from_channel_id(shared_mem.new_data_channel) };
        loop {
            if let Some(data) = reader.pop() {
                break data;
            }
            receiver.receive(executor_context).await;
        }
    }
}

impl Drop for KeyboardBufClient {
    fn drop(&mut self) {
        // TODO: Drop the shared mem
        let _ = self.shared_mem_capability;
    }
}

#[derive(Debug)]
pub struct KeyboardBufServer {
    shared_mem_capability: NonZero<u64>,
    shared_mem: NonNull<[u8]>,
    new_data_sender: Sender,
}

impl KeyboardBufServer {
    pub fn push(&mut self, item: u8) -> Result<(), PushError> {
        let slots_len =
            (self.shared_mem.len() - size_of::<KeyboardBufSharedMem>()) / size_of::<AtomicU8>();
        let shared_mem_ptr = self.shared_mem.cast::<KeyboardBufSharedMem>();
        let shared_mem = unsafe { shared_mem_ptr.as_ref() };
        let shared_slots_ptr = shared_mem.slots.as_ptr();
        let shared_slots = unsafe { slice::from_raw_parts(shared_slots_ptr, slots_len) };
        let mut writer = QueueWriter::new(
            &shared_mem.write_count,
            &shared_mem.read_count,
            shared_slots,
        );
        writer.push(item)?;
        self.new_data_sender.send();
        Ok(())
    }
}

impl Drop for KeyboardBufServer {
    fn drop(&mut self) {
        // TODO: Drop the shared mem
        let _ = self.shared_mem_capability;
    }
}
