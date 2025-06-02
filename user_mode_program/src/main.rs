#![no_std]
#![no_main]

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    let a = [0u8; 0x1000];
    loop {}
}

fn b() {
    let mut a = [0u8; 0x1000];
    unsafe { a.as_mut_ptr().write_volatile(3) };
}

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point() -> ! {
    b();
    todo!()
}
