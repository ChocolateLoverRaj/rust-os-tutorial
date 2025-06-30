#![no_std]
#![no_main]
#![feature(maybe_uninit_slice)]
use core::ops::DerefMut;

use async_keyboard_decoded::AsyncKeyboardDecoded;
use async_mouse_decoded::AsyncMouseDecoded;
use common::{
    SpawnThreadRelativePriority,
    embedded_graphics::{
        pixelcolor::Rgb888,
        prelude::{Dimensions, Point, Size, WebColors},
        primitives::{PrimitiveStyleBuilder, Rectangle, StyledDrawable},
    },
    log,
};
use frame_buffer::FrameBuffer;
use futures::{StreamExt, stream::select};
use pc_keyboard::{HandleControl, KeyCode, KeyState, ScancodeSet1, layouts::Us104Key};
use spawn_process::spawn_process;
use user_lib::{
    ExecutorContext, GuardedStack, execute_future, logger, syscall_exit_process,
    syscall_exit_thread,
};

extern crate alloc;

pub mod async_keyboard_decoded;
pub mod async_mouse_decoded;
pub mod frame_buffer;
pub mod spawn_process;

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    user_lib::panic_handler(info)
}

fn main() {
    logger::init();
    log::info!("Hi");

    spawn_process();

    let mut frame_buffer = FrameBuffer::try_new().unwrap();
    GuardedStack::new(64 * 0x400)
        .unwrap()
        .spawn_thread(worker, SpawnThreadRelativePriority::Lower);
    GuardedStack::new(64 * 0x400)
        .unwrap()
        .spawn_thread(worker, SpawnThreadRelativePriority::Lower);
    let executor_context = ExecutorContext::default();
    execute_future(&executor_context, async {
        enum Input {
            MoveCursor(Point),
            Exit,
        }
        let keyboard = AsyncKeyboardDecoded::new(
            &executor_context,
            ScancodeSet1::default(),
            Us104Key,
            HandleControl::Ignore,
        )
        .map(Result::unwrap)
        .filter_map(async |key| {
            if let KeyState::Down | KeyState::SingleShot = key.state {
                let movement_amount = 5;
                match key.code {
                    KeyCode::ArrowUp => Some(Input::MoveCursor(Point::new(0, -movement_amount))),
                    KeyCode::ArrowDown => Some(Input::MoveCursor(Point::new(0, movement_amount))),
                    KeyCode::ArrowLeft => Some(Input::MoveCursor(Point::new(-movement_amount, 0))),
                    KeyCode::ArrowRight => Some(Input::MoveCursor(Point::new(movement_amount, 0))),
                    KeyCode::Escape | KeyCode::Q => Some(Input::Exit),
                    _ => None,
                }
            } else {
                None
            }
        });
        let mouse = AsyncMouseDecoded::new(&executor_context)
            .map(|stream| {
                stream.map(|packet| {
                    Input::MoveCursor(Point::new(
                        match packet.x_movement() {
                            ps2_mouse::MovementAmount::Overflow(_) => 0,
                            ps2_mouse::MovementAmount::NoOverflow(change) => change as i32,
                        },
                        match packet.y_movement() {
                            ps2_mouse::MovementAmount::Overflow(_) => 0,
                            ps2_mouse::MovementAmount::NoOverflow(change) => -change as i32,
                        },
                    ))
                })
            })
            .ok();
        let mut movement = if let Some(mouse) = mouse {
            select(keyboard, mouse).boxed_local()
        } else {
            keyboard.boxed_local()
        };
        let mut cursor_position = Point::zero();
        frame_buffer
            .bounding_box()
            .draw_styled(
                &PrimitiveStyleBuilder::new()
                    .fill_color(Rgb888::CSS_LIGHT_GRAY)
                    .build(),
                frame_buffer.deref_mut(),
            )
            .unwrap();
        let screen_size = frame_buffer.bounding_box().size;
        Rectangle::new(cursor_position, Size::new(20, 20))
            .draw_styled(
                &PrimitiveStyleBuilder::new()
                    .fill_color(Rgb888::CSS_DARK_GRAY)
                    .build(),
                frame_buffer.deref_mut(),
            )
            .unwrap();
        while let Some(Input::MoveCursor(movement)) = movement.next().await {
            let new_cursor_position = Point::new(
                (cursor_position.x + movement.x)
                    .max(0)
                    .min(screen_size.width as i32),
                (cursor_position.y + movement.y)
                    .max(0)
                    .min(screen_size.height as i32),
            );
            Rectangle::new(cursor_position, Size::new(20, 20))
                .draw_styled(
                    &PrimitiveStyleBuilder::new()
                        .fill_color(Rgb888::CSS_LIGHT_GRAY)
                        .build(),
                    frame_buffer.deref_mut(),
                )
                .unwrap();
            Rectangle::new(new_cursor_position, Size::new(20, 20))
                .draw_styled(
                    &PrimitiveStyleBuilder::new()
                        .fill_color(Rgb888::CSS_DARK_GRAY)
                        .build(),
                    frame_buffer.deref_mut(),
                )
                .unwrap();
            cursor_position = new_cursor_position;
        }
    });
}

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point() -> ! {
    main();
    syscall_exit_process()
}

extern "sysv64" fn worker() -> ! {
    let mut count = 0;
    loop {
        log::debug!("{count}");
        count += 1;
        if count == 1000 {
            break;
        }
    }
    syscall_exit_thread()
}
