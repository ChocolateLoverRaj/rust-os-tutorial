#![no_std]
#![no_main]
#![feature(maybe_uninit_slice)]
#![feature(maybe_uninit_uninit_array_transpose)]
use core::{arch::naked_asm, num::NonZero, ops::DerefMut, ptr::NonNull};

use alloc::vec::Vec;
use common::{
    SpawnProcessRelativePriority, SyscallMapSharedMemError,
    embedded_graphics::{
        Drawable,
        geometry::{AnchorX, AnchorY},
        mono_font::{MonoTextStyleBuilder, iso_8859_3::FONT_10X20},
        pixelcolor::Rgb888,
        prelude::{Dimensions, Point, RgbColor, Size, WebColors},
        primitives::{PrimitiveStyleBuilder, Rectangle, StyledDrawable},
        text::{Baseline, Text},
    },
    log,
};
use frame_buffer::FrameBuffer;
use futures::{FutureExt, StreamExt, pin_mut, select_biased};
use pc_keyboard::{
    HandleControl, KeyCode, KeyEvent, KeyState, Keyboard, ScancodeSet1, layouts::Us104Key,
};
use spawn_process::spawn_process;
use user_lib::{
    AsyncKeyboard, CopyData, EnvEntries, ExecutorContext, KeyboardBufServer,
    KeyboardSharedMemServer, WindowSharedMemServer, execute_future, logger,
};

extern crate alloc;

pub mod async_keyboard_decoded;
// pub mod async_mouse_decoded;
pub mod frame_buffer;
pub mod spawn_process;

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    user_lib::panic_handler(info)
}

fn main(initial_rsp: NonNull<()>) {
    logger::init();
    let env_entries = unsafe { EnvEntries::from_initial_rsp(initial_rsp) };
    log::info!("{env_entries:#X?}");

    let apps = [
        "Keyboard Tester",
        "Non-existent app (will crash if you try to launch)",
    ];

    let mut frame_buffer = FrameBuffer::try_new().unwrap();

    let executor_context = ExecutorContext::default();
    execute_future(&executor_context, async {
        let mut async_keyboard =
            AsyncKeyboard::new(&env_entries, &executor_context, NonZero::new(64).unwrap());
        let mut keyboard = Keyboard::new(ScancodeSet1::new(), Us104Key, HandleControl::Ignore);
        let process_keyboard_data =
            |keyboard: &mut Keyboard<Us104Key, ScancodeSet1>, data: u8| -> Option<KeyEvent> {
                if let Ok(Some(key_event)) = keyboard.add_byte(data) {
                    keyboard.process_keyevent(key_event.clone());
                    Some(key_event)
                } else {
                    None
                }
            };
        struct Tab {
            app_index: usize,
            process_id: NonZero<u32>,
            window: WindowSharedMemServer,
            keyboard_server: KeyboardSharedMemServer,
            keyboard_buffers: Vec<KeyboardBufServer>,
        }
        struct State {
            tabs: Vec<Tab>,
            focus: Focus,
        }
        enum Focus {
            NewTab(usize),
            Tab(usize),
        }
        let mut state = State {
            tabs: Default::default(),
            focus: Focus::NewTab(Default::default()),
        };
        loop {
            let top_bar_height = 40;
            let window_width = frame_buffer.bounding_box().size.width.into();
            let window_height =
                u64::from(frame_buffer.bounding_box().size.height) - u64::from(top_bar_height);
            let top_bar_rect = frame_buffer
                .bounding_box()
                .resized_height(top_bar_height, AnchorY::Top);
            top_bar_rect
                .draw_styled(
                    &PrimitiveStyleBuilder::new()
                        .fill_color(Rgb888::new(20, 20, 20))
                        .build(),
                    frame_buffer.deref_mut(),
                )
                .unwrap();
            let new_tab_width = top_bar_height;
            let new_tab_rect = top_bar_rect.resized_width(new_tab_width, AnchorX::Left);
            new_tab_rect
                .draw_styled(
                    &PrimitiveStyleBuilder::new()
                        .fill_color(if let Focus::NewTab(_) = state.focus {
                            Rgb888::CSS_GREEN
                        } else {
                            Rgb888::CSS_DARK_GREEN
                        })
                        .build(),
                    frame_buffer.deref_mut(),
                )
                .unwrap();
            let mut position = new_tab_width;
            for (index, tab) in state.tabs.iter().enumerate() {
                let font = &FONT_10X20;
                let app_name = apps[tab.app_index];
                // Note that we are using the number of bytes, which will overestimate the width for characters that are >1 bytes.
                // However, the font we are using can't even display those kinds of characters so this logic is fine for now.
                let text_width = font.character_size.width * u32::try_from(app_name.len()).unwrap()
                    + font.character_spacing * u32::try_from(app_name.len() - 1).unwrap();
                let padding_x = 10;
                let tab_rect_width = padding_x + text_width + padding_x;
                let tab_rect = Rectangle::new(
                    Point::new(position.try_into().unwrap(), 0),
                    Size::new(tab_rect_width, top_bar_height),
                );
                let is_focused = if let Focus::Tab(focused_tab_index) = state.focus
                    && focused_tab_index == index
                {
                    true
                } else {
                    false
                };
                tab_rect
                    .draw_styled(
                        &PrimitiveStyleBuilder::new()
                            .fill_color(if is_focused {
                                Rgb888::new(40, 40, 40)
                            } else {
                                Rgb888::new(30, 30, 30)
                            })
                            .build(),
                        frame_buffer.deref_mut(),
                    )
                    .unwrap();
                Text::with_baseline(
                    app_name,
                    Point::new(
                        (position + padding_x).try_into().unwrap(),
                        (top_bar_height / 2).try_into().unwrap(),
                    ),
                    {
                        let mut builder = MonoTextStyleBuilder::new()
                            .font(font)
                            .text_color(Rgb888::WHITE);
                        if is_focused {
                            builder = builder.underline();
                        }
                        builder.build()
                    },
                    Baseline::Middle,
                )
                .draw(frame_buffer.deref_mut())
                .unwrap();
                position += tab_rect_width;
            }
            let content_rect = frame_buffer.bounding_box().resized_height(
                frame_buffer.bounding_box().size.height - top_bar_height,
                AnchorY::Bottom,
            );

            match &mut state.focus {
                Focus::NewTab(focused_index) => {
                    let font = &FONT_10X20;
                    apps.iter().enumerate().for_each(|(index, app)| {
                        Text::with_baseline(
                            app,
                            Point::new(
                                0,
                                i32::try_from(top_bar_height).unwrap()
                                    + i32::try_from(font.character_size.height).unwrap()
                                        * i32::try_from(index).unwrap(),
                            ),
                            MonoTextStyleBuilder::new()
                                .font(font)
                                .text_color(if index == *focused_index {
                                    Rgb888::BLACK
                                } else {
                                    Rgb888::WHITE
                                })
                                .background_color(if index == *focused_index {
                                    Rgb888::WHITE
                                } else {
                                    Rgb888::BLACK
                                })
                                .build(),
                            Baseline::Top,
                        )
                        .draw(frame_buffer.deref_mut())
                        .unwrap();
                    });
                    loop {
                        let key_event = loop {
                            let data = async_keyboard.next().await.unwrap();
                            if let Some(key_event) = process_keyboard_data(&mut keyboard, data) {
                                break key_event;
                            }
                        };
                        if let KeyState::Down | KeyState::SingleShot = key_event.state {
                            match key_event.code {
                                KeyCode::ArrowUp => {
                                    let new_focused_index =
                                        focused_index.checked_sub(1).unwrap_or(apps.len() - 1);
                                    *focused_index = new_focused_index;
                                    break;
                                }
                                KeyCode::ArrowDown => {
                                    let new_focused_index = if *focused_index + 1 < apps.len() {
                                        *focused_index + 1
                                    } else {
                                        0
                                    };
                                    *focused_index = new_focused_index;
                                    break;
                                }
                                KeyCode::Return => {
                                    let mut send_capabilities = Vec::new();
                                    let (
                                        window,
                                        window_shared_mem_capability,
                                        window_send_capability,
                                    ) = WindowSharedMemServer::new(
                                        window_width,
                                        window_height,
                                        &frame_buffer,
                                    )
                                    .unwrap();
                                    send_capabilities.push(window_shared_mem_capability);
                                    send_capabilities.push(window_send_capability);
                                    let (
                                        keyboard_server,
                                        keyboard_shared_mem_capability,
                                        keyboard_send_capability0,
                                        keyboard_send_capability1,
                                    ) = KeyboardSharedMemServer::new().unwrap();
                                    send_capabilities.push(keyboard_shared_mem_capability);
                                    send_capabilities.push(keyboard_send_capability0);
                                    send_capabilities.push(keyboard_send_capability1);
                                    let process_id = spawn_process(
                                        *focused_index,
                                        SpawnProcessRelativePriority::Lower,
                                        window_shared_mem_capability,
                                        keyboard_shared_mem_capability,
                                        &send_capabilities,
                                    );
                                    log::info!("Spawned process");
                                    state.tabs.push(Tab {
                                        app_index: *focused_index,
                                        process_id,
                                        window,
                                        keyboard_server,
                                        keyboard_buffers: Default::default(),
                                    });
                                    state.focus = Focus::Tab(state.tabs.len() - 1);
                                    break;
                                }
                                KeyCode::Escape | KeyCode::W => {
                                    if !state.tabs.is_empty() {
                                        state.focus = Focus::Tab(0);
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Focus::Tab(tab_index) => {
                    let tab_content_rect = frame_buffer.bounding_box().resized_height(
                        frame_buffer.bounding_box().size.height - top_bar_height,
                        AnchorY::Bottom,
                    );
                    tab_content_rect
                        .draw_styled(
                            &PrimitiveStyleBuilder::new()
                                .fill_color(Rgb888::BLACK)
                                .build(),
                            frame_buffer.deref_mut(),
                        )
                        .unwrap();
                    let window_server = &mut state.tabs[*tab_index].window;
                    window_server.copy_to_frame_buffer(
                        CopyData {
                            x: 0,
                            y: 0,
                            width: window_width,
                            height: window_height,
                        },
                        &mut frame_buffer,
                        0,
                        top_bar_height.into(),
                    );
                    loop {
                        enum Event {
                            KeyboardRequest(Result<KeyboardBufServer, SyscallMapSharedMemError>),
                            KeyboardData(Option<u8>),
                            Draw,
                        }
                        let event = {
                            let tab = &mut state.tabs[*tab_index];
                            let client_process_id = tab.process_id;
                            let keyboard_request_fut = tab
                                .keyboard_server
                                .wait_for_request(&executor_context, client_process_id)
                                .fuse();
                            pin_mut!(keyboard_request_fut);
                            let mut keyboard_data_fut = async_keyboard.next();
                            let draw_fut = tab
                                .window
                                .handle_draw_request(
                                    &executor_context,
                                    &mut frame_buffer,
                                    0,
                                    top_bar_height.into(),
                                )
                                .fuse();
                            pin_mut!(draw_fut);

                            select_biased! {
                                result = keyboard_request_fut => Event::KeyboardRequest(result),
                                keyboard_data = keyboard_data_fut => Event::KeyboardData(keyboard_data),
                                _ = draw_fut => Event::Draw
                            }
                        };
                        match event {
                            Event::KeyboardRequest(result) => match result {
                                Ok(keyboard_buf) => {
                                    state.tabs[*tab_index].keyboard_buffers.push(keyboard_buf);
                                }
                                Err(e) => log::warn!("Error getting keyboard request: {e:#?}"),
                            },
                            Event::KeyboardData(data) => {
                                let data = data.unwrap();
                                if let Some(key_event) = process_keyboard_data(&mut keyboard, data)
                                    && let KeyState::Down | KeyState::SingleShot = key_event.state
                                    && keyboard.get_modifiers().is_ctrl()
                                {
                                    match key_event.code {
                                        KeyCode::W => {
                                            let tab_index = *tab_index;
                                            state.tabs.remove(tab_index);
                                            state.focus = if !state.tabs.is_empty() {
                                                Focus::Tab(tab_index.saturating_sub(1))
                                            } else {
                                                Focus::NewTab(Default::default())
                                            };
                                            content_rect
                                                .draw_styled(
                                                    &PrimitiveStyleBuilder::new()
                                                        .fill_color(Rgb888::BLACK)
                                                        .build(),
                                                    frame_buffer.deref_mut(),
                                                )
                                                .unwrap();
                                            break;
                                        }
                                        KeyCode::T => {
                                            state.focus = Focus::NewTab(Default::default());
                                            content_rect
                                                .draw_styled(
                                                    &PrimitiveStyleBuilder::new()
                                                        .fill_color(Rgb888::BLACK)
                                                        .build(),
                                                    frame_buffer.deref_mut(),
                                                )
                                                .unwrap();
                                            break;
                                        }
                                        KeyCode::Tab => {
                                            *tab_index = {
                                                #[allow(clippy::collapsible_else_if)]
                                                if keyboard.get_modifiers().is_shifted() {
                                                    if *tab_index == 0 {
                                                        state.tabs.len() - 1
                                                    } else {
                                                        *tab_index - 1
                                                    }
                                                } else {
                                                    if *tab_index + 1 < state.tabs.len() {
                                                        *tab_index + 1
                                                    } else {
                                                        0
                                                    }
                                                }
                                            };
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                for buf in &mut state.tabs[*tab_index].keyboard_buffers {
                                    let _ = buf.push(data);
                                }
                            }
                            Event::Draw => {
                                log::debug!("Drew app rectangle to screen");
                            }
                        }
                    }
                }
            }
        }
    });
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
