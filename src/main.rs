mod draw_mode;
mod layout;
mod touch_mode;

use crate::layout::{canvas_handlers::handle_draw_event, CANVAS_REGION};
use draw_mode::DrawMode;
use libremarkable::{
    appctx,
    framebuffer::{cgmath, common::*, FramebufferDraw, FramebufferRefresh, PartialRefreshMode},
    image, input,
    input::InputEvent,
    ui_extensions::element::{UIConstraintRefresh, UIElement, UIElementHandle, UIElementWrapper},
};
/* use touch_mode::TouchMode; */

#[cfg(feature = "enable-runtime-benchmarking")]
use libremarkable::stopwatch;

use atomic::Atomic;
use once_cell::sync::Lazy;

use std::collections::VecDeque;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

const APP_BUTTON_TOP: u16 = 68;
const APP_DROPDOWN_TOP: u16 = 180;
const APP_BUTTON_GAP_H: u16 = 10;
const APP_BUTTON_GAP_V: u16 = 10;
const APP_BUTTON_WIDTH: u16 = 32;
const APP_BUTTON_SPACE_H: u16 = APP_BUTTON_WIDTH + (APP_BUTTON_GAP_H * 2);
const APP_BUTTON_SPACE_V: u16 = APP_BUTTON_WIDTH + (APP_BUTTON_GAP_V * 2);
const DISPLAY_EDGE_TOP: u16 = 0;
const DISPLAY_EDGE_RIGHT: u16 = DISPLAYWIDTH;

/* static G_TOUCH_MODE: Lazy<Atomic<TouchMode>> = Lazy::new(|| Atomic::new(TouchMode::OnlyUI)); */
static G_DRAW_MODE: Lazy<Atomic<DrawMode>> = Lazy::new(|| Atomic::new(DrawMode::Draw(2)));
static UNPRESS_OBSERVED: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
static WACOM_IN_RANGE: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
static WACOM_RUBBER_SIDE: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
static WACOM_HISTORY: Lazy<Mutex<VecDeque<(cgmath::Point2<f32>, i32)>>> =
    Lazy::new(|| Mutex::new(VecDeque::new()));

// ####################
// ## Input Handlers
// ####################

fn on_wacom_input(app: &mut appctx::ApplicationContext<'_>, input: input::WacomEvent) {
    match input {
        input::WacomEvent::Draw {
            position,
            pressure,
            tilt,
        } => {
            handle_draw_event(app.upgrade_ref(), position, pressure, tilt);
        }
        input::WacomEvent::InstrumentChange { pen, state } => {
            match pen {
                // Whether the pen is in range
                input::WacomPen::ToolPen => {
                    WACOM_IN_RANGE.store(state, Ordering::Relaxed);
                    WACOM_RUBBER_SIDE.store(false, Ordering::Relaxed);
                }
                input::WacomPen::ToolRubber => {
                    WACOM_IN_RANGE.store(state, Ordering::Relaxed);
                    WACOM_RUBBER_SIDE.store(true, Ordering::Relaxed);
                }
                // Whether the pen is actually making contact
                input::WacomPen::Touch => {
                    // Stop drawing when instrument has left the vicinity of the screen
                    if !state {
                        let mut wacom_stack = WACOM_HISTORY.lock().unwrap();
                        wacom_stack.clear();
                    }
                }
                _ => unreachable!(),
            }
        }
        input::WacomEvent::Hover {
            position: _,
            distance,
            tilt: _,
        } => {
            // If the pen is hovering, don't record its coordinates as the origin of the next line
            if distance > 1 {
                let mut wacom_stack = WACOM_HISTORY.lock().unwrap();
                wacom_stack.clear();
                UNPRESS_OBSERVED.store(true, Ordering::Relaxed);
            }
        }
        _ => {}
    };
}

fn on_touch_handler(_app: &mut appctx::ApplicationContext<'_>, input: input::MultitouchEvent) {
    /* let framebuffer = app.get_framebuffer_ref(); */
    match input {
        input::MultitouchEvent::Press { finger } | input::MultitouchEvent::Move { finger } => {
            if !CANVAS_REGION.contains_point(&finger.pos.cast().unwrap()) {
                return;
            }
            /* let rect = match G_TOUCH_MODE.load(Ordering::Relaxed) {
                TouchMode::Bezier => {
                    let position_float = finger.pos.cast().unwrap();
                    let points = vec![
                        (cgmath::vec2(-40.0, 0.0), 2.5),
                        (cgmath::vec2(40.0, -60.0), 5.5),
                        (cgmath::vec2(0.0, 0.0), 3.5),
                        (cgmath::vec2(-40.0, 60.0), 6.5),
                        (cgmath::vec2(-10.0, 50.0), 5.0),
                        (cgmath::vec2(10.0, 45.0), 4.5),
                        (cgmath::vec2(30.0, 55.0), 3.5),
                        (cgmath::vec2(50.0, 65.0), 3.0),
                        (cgmath::vec2(70.0, 40.0), 0.0),
                    ];
                    let mut rect = mxcfb_rect::invalid();
                    for window in points.windows(3).step_by(2) {
                        rect = rect.merge_rect(&framebuffer.draw_dynamic_bezier(
                            (position_float + window[0].0, window[0].1),
                            (position_float + window[1].0, window[1].1),
                            (position_float + window[2].0, window[2].1),
                            100,
                            color::BLACK,
                        ));
                    }
                    rect
                }
                TouchMode::Circles => {
                    framebuffer.draw_circle(finger.pos.cast().unwrap(), 20, color::BLACK)
                }

                m @ TouchMode::Diamonds | m @ TouchMode::FillDiamonds => {
                    let position_int = finger.pos.cast().unwrap();
                    framebuffer.draw_polygon(
                        &[
                            position_int + cgmath::vec2(-10, 0),
                            position_int + cgmath::vec2(0, 20),
                            position_int + cgmath::vec2(10, 0),
                            position_int + cgmath::vec2(0, -20),
                        ],
                        match m {
                            /* TouchMode::Diamonds => false,
                            TouchMode::FillDiamonds => true, */
                            _ => false,
                        },
                        color::BLACK,
                    )
                }
                _ => return,
            };
            framebuffer.partial_refresh(
                &rect,
                PartialRefreshMode::Async,
                waveform_mode::WAVEFORM_MODE_DU,
                display_temp::TEMP_USE_REMARKABLE_DRAW,
                dither_mode::EPDC_FLAG_USE_DITHERING_ALPHA,
                DRAWING_QUANT_BIT,
                false,
            ); */
        }
        _ => {}
    }
}

fn on_button_press(input: input::GPIOEvent) {
    let (btn, new_state) = match input {
        input::GPIOEvent::Press { button } => (button, true),
        input::GPIOEvent::Unpress { button } => (button, false),
        _ => return,
    };

    // Ignoring the unpressed event
    if !new_state {
        return;
    }

    // Simple but effective accidental button press filtering
    if WACOM_IN_RANGE.load(Ordering::Relaxed) {
        return;
    }

    match btn {
        input::PhysicalButton::POWER => {
            Command::new("systemctl")
                .arg("start")
                .arg("xochitl")
                .spawn()
                .unwrap();
            std::process::exit(0);
        }
        input::PhysicalButton::WAKEUP => {
            println!("WAKEUP button(?) pressed(?)");
        }
        _ => {}
    };
}

fn main() {
    // Create the logger
    env_logger::init();

    // Takes callback functions as arguments
    // They are called with the event and the &mut framebuffer
    let mut app: appctx::ApplicationContext<'_> = appctx::ApplicationContext::default();

    // Perform the initial layout
    layout::init(&mut app);

    // Draw the scene
    app.draw_elements();

    // Blocking call to process events from digitizer + touchscreen + physical buttons
    app.start_event_loop(true, true, true, |ctx, evt| match evt {
        InputEvent::WacomEvent { event } => on_wacom_input(ctx, event),
        InputEvent::MultitouchEvent { event } => on_touch_handler(ctx, event),
        InputEvent::GPIO { event } => on_button_press(event),
        _ => {}
    });
}
