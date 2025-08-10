#![no_std]
#![no_main]

use core::{arch::naked_asm, num::NonZero, ptr::NonNull};

use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts::Us104Key};
use spin::Once;
use user_lib::{
    EnvEntries, ExecutorContext, GuardedStack, KeyboardSharedMemClient, MemProt, PageSize,
    SpawnThreadRelativePriority, SyscallMemProt, SyscallMemProtInput, execute_future, log, logger,
    syscall, syscall_alloc, syscall_exit_process,
};

static ALLOCATED_MEM: Once<NonZero<usize>> = Once::new();

fn main(initial_rsp: NonNull<()>) -> ! {
    logger::init();
    let env_entries = unsafe { EnvEntries::from_initial_rsp(initial_rsp) };
    log::debug!("{env_entries:#X?}");

    let mut keyboard = unsafe { KeyboardSharedMemClient::new(&env_entries) }.unwrap();
    let executor_context = ExecutorContext::default();
    let pages_len = NonZero::new(1).unwrap();
    let mem = ALLOCATED_MEM.call_once(|| {
        syscall_alloc(
            PageSize::_4KiB,
            pages_len,
            false,
            MemProt::READABLE | MemProt::WRITABLE,
        )
        .unwrap()
        .addr()
    });
    log::debug!("Spawning other thread");
    GuardedStack::new(64 * 0x400)
        .unwrap()
        .spawn_thread(other_thread, SpawnThreadRelativePriority::Lower);
    execute_future(&executor_context, async {
        let mut client = keyboard
            .request(&executor_context, 64.try_into().unwrap())
            .await
            .unwrap();
        let mut keyboard = Keyboard::new(ScancodeSet1::new(), Us104Key, HandleControl::Ignore);
        loop {
            let data = client.read(&executor_context).await;
            if let Ok(Some(event)) = keyboard.add_byte(data)
                && let Some(key) = keyboard.process_keyevent(event)
                && let DecodedKey::Unicode('u') = key
            {
                break;
            }
        }
    });
    let input = SyscallMemProtInput {
        start_page_index: NonZero::new(mem.get() / PageSize::_4KiB.byte_len()).unwrap(),
        new_prot: MemProt::READABLE.bits(),
        page_size: PageSize::_4KiB,
        pages_len,
    };
    unsafe { syscall::<SyscallMemProt>(&input) }.unwrap();
    log::debug!("Changed mem prot");
    syscall_exit_process()
}

extern "sysv64" fn other_thread() -> ! {
    let mem = ALLOCATED_MEM.get().unwrap().get() as *mut u8;
    let mut n = 0;
    loop {
        unsafe { mem.write_volatile(Default::default()) };
        log::debug!("write {n}");
        n += 1;
    }
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
unsafe extern "sysv64" fn entry_point() -> ! {
    naked_asm!(
        "
            mov rdi, rsp
            call {main}
        ",
        main = sym main
    )
}

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    user_lib::panic_handler(info)
}
