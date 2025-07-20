#![no_std]
#![no_main]
#![feature(maybe_uninit_slice)]
#![feature(maybe_uninit_uninit_array_transpose)]
use core::{arch::naked_asm, ops::DerefMut, ptr::NonNull};

use alloc::vec::Vec;
use async_keyboard_decoded::AsyncKeyboardDecoded;
use common::{
    SpawnProcessRelativePriority,
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
use futures::StreamExt;
use pc_keyboard::{HandleControl, KeyCode, KeyState, ScancodeSet1, layouts::Us104Key};
use spawn_process::spawn_process;
use user_lib::{
    AsyncKeyboard, EnvEntries, ExecutorContext, KeyboardSharedMemServer, WindowSharedMemServer,
    execute_future, logger,
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

fn main(initial_rsp: NonNull<()>) {
    logger::init();
    let env_entries = unsafe { EnvEntries::from_initial_rsp(initial_rsp) };
    log::info!("{env_entries:#X?}");

    let apps = [
        "Test App",
        "Non-existent app (will crash if you try to launch)",
    ];

    let mut frame_buffer = FrameBuffer::try_new().unwrap();

    let executor_context = ExecutorContext::default();
    execute_future(&executor_context, async {
        let mut async_keyboard = AsyncKeyboardDecoded::new(
            AsyncKeyboard::new(&env_entries, &executor_context, 64),
            ScancodeSet1::new(),
            Us104Key,
            HandleControl::Ignore,
        );
        struct Tab {
            app_index: usize,
            window: WindowSharedMemServer,
            keyboard_server: KeyboardSharedMemServer,
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
                // TODO: Get UTF-8 character count and not byte-count
                let app_name = apps[tab.app_index];
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
                        let key_event = async_keyboard.next().await.unwrap().unwrap();
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
                                    let width =
                                        frame_buffer.bounding_box().size.width.try_into().unwrap();
                                    let height =
                                        u64::try_from(frame_buffer.bounding_box().size.height)
                                            .unwrap()
                                            - u64::from(top_bar_height);
                                    let window =
                                        WindowSharedMemServer::new(width, height, &frame_buffer);
                                    let keyboard_server = KeyboardSharedMemServer::new().unwrap();
                                    spawn_process(
                                        *focused_index,
                                        SpawnProcessRelativePriority::Lower,
                                        &window,
                                        &keyboard_server,
                                    );
                                    state.tabs.push(Tab {
                                        app_index: *focused_index,
                                        window,
                                        keyboard_server,
                                    });
                                    state.focus = Focus::Tab(0);
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
                    state.tabs[*tab_index].window.draw_to_frame_buffer(
                        &mut frame_buffer,
                        0,
                        top_bar_height.into(),
                    );
                    loop {
                        let s = state.tabs[*tab_index]
                            .keyboard_server
                            .wait_for_request(&executor_context)
                            .await;
                        log::debug!("requested slots count: {s:?}");
                        let key_event = async_keyboard.next().await.unwrap().unwrap();
                        if let KeyState::Down | KeyState::SingleShot = key_event.state {
                            match key_event.code {
                                KeyCode::W | KeyCode::Escape => {
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
                                    *tab_index = if *tab_index + 1 < state.tabs.len() {
                                        *tab_index + 1
                                    } else {
                                        0
                                    };
                                    break;
                                }
                                _ => {}
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
