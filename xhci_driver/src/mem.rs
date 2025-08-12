// From https://github.com/FlareCoding/stellux-xhci-tutorial/blob/b08162d6032c6927be8282a810ab54326d633f4b/kernel/include/drivers/usb/xhci/xhci_mem.h

use core::num::NonZero;

pub const PAGE_SIZE: NonZero<u64> = NonZero::new(0x1000).unwrap();

// Max sizes
pub const XHCI_DEVICE_CONTEXT_INDEX_MAX_SIZE: NonZero<u64> = NonZero::new(2048).unwrap();
pub const XHCI_DEVICE_CONTEXT_MAX_SIZE: NonZero<u64> = NonZero::new(2048).unwrap();
pub const XHCI_INPUT_CONTROL_CONTEXT_MAX_SIZE: NonZero<u64> = NonZero::new(64).unwrap();
pub const XHCI_SLOT_CONTEXT_MAX_SIZE: NonZero<u64> = NonZero::new(64).unwrap();
pub const XHCI_ENDPOINT_CONTEXT_MAX_SIZE: NonZero<u64> = NonZero::new(64).unwrap();
pub const XHCI_STREAM_CONTEXT_MAX_SIZE: NonZero<u64> = NonZero::new(16).unwrap();
pub const XHCI_STREAM_ARRAY_LINEAR_MAX_SIZE: NonZero<u64> = NonZero::new(1024 * 1024).unwrap(); // 1 MB
pub const XHCI_STREAM_ARRAY_PRI_SEC_MAX_SIZE: NonZero<u64> = PAGE_SIZE;
pub const XHCI_TRANSFER_RING_SEGMENTS_MAX_SIZE: NonZero<u64> = NonZero::new(1024 * 64).unwrap(); // 64 KB
pub const XHCI_COMMAND_RING_SEGMENTS_MAX_SIZE: NonZero<u64> = NonZero::new(1024 * 64).unwrap(); // 64 KB
pub const XHCI_EVENT_RING_SEGMENTS_MAX_SIZE: NonZero<u64> = NonZero::new(1024 * 64).unwrap(); // 64 KB
pub const XHCI_EVENT_RING_SEGMENT_TABLE_MAX_SIZE: NonZero<u64> = NonZero::new(1024 * 512).unwrap(); // 512 KB
pub const XHCI_SCRATCHPAD_BUFFER_ARRAY_MAX_SIZE: NonZero<u64> = NonZero::new(248).unwrap();
pub const XHCI_SCRATCHPAD_BUFFERS_MAX_SIZE: NonZero<u64> = PAGE_SIZE;

// Boundaries
pub const XHCI_DEVICE_CONTEXT_INDEX_BOUNDARY: NonZero<u64> = PAGE_SIZE;
pub const XHCI_DEVICE_CONTEXT_BOUNDARY: NonZero<u64> = PAGE_SIZE;
pub const XHCI_INPUT_CONTROL_CONTEXT_BOUNDARY: NonZero<u64> = PAGE_SIZE;
pub const XHCI_SLOT_CONTEXT_BOUNDARY: NonZero<u64> = PAGE_SIZE;
pub const XHCI_ENDPOINT_CONTEXT_BOUNDARY: NonZero<u64> = PAGE_SIZE;
pub const XHCI_STREAM_CONTEXT_BOUNDARY: NonZero<u64> = PAGE_SIZE;
pub const XHCI_STREAM_ARRAY_LINEAR_BOUNDARY: NonZero<u64> = PAGE_SIZE;
pub const XHCI_STREAM_ARRAY_PRI_SEC_BOUNDARY: NonZero<u64> = PAGE_SIZE;
pub const XHCI_TRANSFER_RING_SEGMENTS_BOUNDARY: NonZero<u64> = NonZero::new(1024 * 64).unwrap(); // 64 KB
pub const XHCI_COMMAND_RING_SEGMENTS_BOUNDARY: NonZero<u64> = NonZero::new(1024 * 64).unwrap(); // 64 KB
pub const XHCI_EVENT_RING_SEGMENTS_BOUNDARY: NonZero<u64> = NonZero::new(1024 * 64).unwrap(); // 64 KB
pub const XHCI_EVENT_RING_SEGMENT_TABLE_BOUNDARY: NonZero<u64> = PAGE_SIZE;
pub const XHCI_SCRATCHPAD_BUFFER_ARRAY_BOUNDARY: NonZero<u64> = PAGE_SIZE;
pub const XHCI_SCRATCHPAD_BUFFERS_BOUNDARY: NonZero<u64> = PAGE_SIZE;

// Alignments
pub const XHCI_DEVICE_CONTEXT_INDEX_ALIGNMENT: NonZero<u64> = NonZero::new(64).unwrap();
pub const XHCI_DEVICE_CONTEXT_ALIGNMENT: NonZero<u64> = NonZero::new(64).unwrap();
pub const XHCI_INPUT_CONTROL_CONTEXT_ALIGNMENT: NonZero<u64> = NonZero::new(64).unwrap();
pub const XHCI_SLOT_CONTEXT_ALIGNMENT: NonZero<u64> = NonZero::new(32).unwrap();
pub const XHCI_ENDPOINT_CONTEXT_ALIGNMENT: NonZero<u64> = NonZero::new(32).unwrap();
pub const XHCI_STREAM_CONTEXT_ALIGNMENT: NonZero<u64> = NonZero::new(16).unwrap();
pub const XHCI_STREAM_ARRAY_LINEAR_ALIGNMENT: NonZero<u64> = NonZero::new(16).unwrap();
pub const XHCI_STREAM_ARRAY_PRI_SEC_ALIGNMENT: NonZero<u64> = NonZero::new(16).unwrap();
pub const XHCI_TRANSFER_RING_SEGMENTS_ALIGNMENT: NonZero<u64> = NonZero::new(64).unwrap();
pub const XHCI_COMMAND_RING_SEGMENTS_ALIGNMENT: NonZero<u64> = NonZero::new(64).unwrap();
pub const XHCI_EVENT_RING_SEGMENTS_ALIGNMENT: NonZero<u64> = NonZero::new(64).unwrap();
pub const XHCI_EVENT_RING_SEGMENT_TABLE_ALIGNMENT: NonZero<u64> = NonZero::new(64).unwrap();
pub const XHCI_SCRATCHPAD_BUFFER_ARRAY_ALIGNMENT: NonZero<u64> = NonZero::new(64).unwrap();
pub const XHCI_SCRATCHPAD_BUFFERS_ALIGNMENT: NonZero<u64> = PAGE_SIZE;
