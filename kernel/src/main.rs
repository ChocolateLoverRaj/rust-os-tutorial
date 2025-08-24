#![no_std]
#![no_main]

use core::fmt::Write;

use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, RgbColor},
};
use frame_buffer_embedded_graphics::*;
use frame_buffer_info::*;
use limine_requests::*;
use rgb_pixel_info::*;
use uart_16550::SerialPort;

mod frame_buffer_embedded_graphics;
mod frame_buffer_info;
mod limine_requests;
mod rgb_pixel_info;

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point_bsp() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    assert!(BASE_REVISION.is_supported());

    let mut serial_port = unsafe { SerialPort::new(0x3F8) };
    serial_port.init();
    writeln!(serial_port, "Hello World!\r").unwrap();

    let frame_buffer = FRAME_BUFFER_REQUEST.get_response().unwrap();
    if let Some(frame_buffer) = frame_buffer.framebuffers().next() {
        let mut frame_buffer = {
            let addr = frame_buffer.addr().addr().try_into().unwrap();
            let info = (&frame_buffer).into();
            unsafe { FrameBufferEmbeddedGraphics::new(addr, info) }
        };
        frame_buffer.clear(Rgb888::MAGENTA).unwrap();
    }

    hlt_loop();
}

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    hlt_loop();
}

fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
