use core::{num::NonZero, sync::atomic::AtomicU8};

use common::{
    EventStreamMem, LOWER_HALF_END, PageSize, SyscallSubscribeToKeyboard,
    SyscallSubscribeToKeyboardError, SyscallSubscribeToKeyboardOutput,
};
use nodit::{InclusiveInterval, Interval, interval::ee};
use x86_64::{VirtAddr, registers::model_specific::PatMemoryType};

use crate::{
    Capability, CapabilityId, ConfigurableFlags, EventStream, EventStreamSource, MapPageError2,
    Page,
    capabilities::{CAPABILITIES, CapabilityType},
    cpu_local_data::get_local,
    memory::{MEMORY, MemoryType},
    task::{THREADS, UserVirtMem},
    translate_addr::ZeroFrame,
};

use super::GenericSyscallHandler;

pub struct SyscallSubscribeToKeyboardHandler;
impl GenericSyscallHandler for SyscallSubscribeToKeyboardHandler {
    type S = SyscallSubscribeToKeyboard;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = (|| {
            let threads = THREADS.read();
            let local = get_local();
            let current_process = &threads
                .get(&local.running_thread.lock().unwrap())
                .unwrap()
                .process;

            let total_size = size_of::<EventStreamMem>()
                .checked_add(size_of::<AtomicU8>() * helper.input().slots_len.get())
                .ok_or(SyscallSubscribeToKeyboardError::InvalidSlotsLen)?;
            let page_size = PageSize::_4KiB;
            let pages_len = total_size.div_ceil(page_size.byte_len());
            let slots_len = (pages_len * page_size.byte_len() - size_of::<EventStreamMem>())
                / size_of::<AtomicU8>();

            // Check permissions
            let mut capabilities = CAPABILITIES.write();
            let capability = capabilities
                .get(&helper.input().capability)
                .ok_or(SyscallSubscribeToKeyboardError::CapabilityNotFound)?;
            if capability.process_id != current_process.id.into() {
                return Err(SyscallSubscribeToKeyboardError::CapabilityNotFound);
            }
            if !matches!(capability._type, CapabilityType::Ps2Keyboard) {
                Err(SyscallSubscribeToKeyboardError::InvalidCapability)?;
            }

            let capability_id = CapabilityId::new_unique();

            // Allocate virt mem in the process's addr space
            let mut mem = current_process.memory.write();
            let interval = mem
                .mapped_virtual_memory
                .gaps_trimmed(ee(0, LOWER_HALF_END))
                .find_map(|gap| {
                    let start = gap.start().next_multiple_of(PageSize::_4KiB.byte_len_u64());
                    let interval =
                        Interval::from(start..start + pages_len as u64 * page_size.byte_len_u64());
                    if gap.contains_interval(&interval) {
                        Some(interval)
                    } else {
                        None
                    }
                })
                .ok_or(SyscallSubscribeToKeyboardError::OutOfVirtMem)?;
            mem.mapped_virtual_memory
                .insert_strict(interval, UserVirtMem::EventStream(capability_id))
                .unwrap();

            // Allocate virt mem in the kernel's addr space
            let memory = MEMORY.get().unwrap();
            let mut virt_mem = memory.virtual_memory.lock();
            let mut kernel_pages = if let Some(kernel_pages) =
                virt_mem.allocate_contiguous_pages(PageSize::_4KiB, pages_len as u64)
            {
                kernel_pages
            } else {
                // TODO: Clean up previous
                return Err(SyscallSubscribeToKeyboardError::OutOfKernelVirtMem);
            };

            // Allocate phys mem and map it
            let mut phys_mem = memory.physical_memory.lock();
            for i in 0..pages_len {
                let frame = if let Some(frame) = phys_mem.allocate_frame_with_type(
                    page_size,
                    MemoryType::UsedByUserMode(current_process.id),
                ) {
                    frame
                } else {
                    // TODO: Clean up previous
                    return Err(SyscallSubscribeToKeyboardError::OutOfPhysMem);
                };
                // Zero the frame
                unsafe { frame.zero() };
                // Map in user addr space
                {
                    let page = Page::new(VirtAddr::new(interval.start()), page_size)
                        .unwrap()
                        .offset(i as u64)
                        .unwrap();
                    let flags = ConfigurableFlags {
                        writable: true,
                        executable: false,
                        pat_memory_type: PatMemoryType::WriteBack,
                    };
                    let mut frame_allocator =
                        phys_mem.get_user_mode_program_frame_allocator(current_process.id);
                    let result =
                        unsafe { mem.l4.map_page(page, frame, flags, &mut frame_allocator) };
                    if let Err(e) = &result {
                        match e {
                            MapPageError2::FrameAllocationFailed => {
                                // TODO: Clean up previous
                                return Err(SyscallSubscribeToKeyboardError::OutOfPhysMem);
                            }
                            _ => result.unwrap(),
                        }
                    }
                }
                // Map in kernel addr space
                {
                    let page = Page::new(VirtAddr::new(*kernel_pages.range().start()), page_size)
                        .unwrap()
                        .offset(i as u64)
                        .unwrap();
                    let flags = ConfigurableFlags {
                        writable: true,
                        executable: false,
                        pat_memory_type: PatMemoryType::WriteBack,
                    };
                    let mut frame_allocator = phys_mem.get_kernel_frame_allocator();
                    if let Err(_e) =
                        unsafe { kernel_pages.map_to(page, frame, flags, &mut frame_allocator) }
                    {
                        // TODO: Clean up previous
                        return Err(SyscallSubscribeToKeyboardError::OutOfPhysMem);
                    }
                }
            }

            capabilities.insert(
                capability_id.into(),
                Capability {
                    _type: CapabilityType::EventStream(EventStream {
                        process: current_process.clone(),
                        source: EventStreamSource::Ps2Keyboard,
                        ptr: NonZero::new(*kernel_pages.range().start() as usize).unwrap(),
                        slots_len,
                    }),
                    process_id: current_process.id.into(),
                },
            );
            Ok(SyscallSubscribeToKeyboardOutput {
                addr: (interval.start() as usize).try_into().unwrap(),
                event: capability_id.into(),
                slots_len: slots_len.try_into().unwrap(),
            })
        })();
        helper.syscall_return(&output)
    }
}
