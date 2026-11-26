use anyhow::{Context, Result};
use tray_icon::{
    Icon, MouseButtonState, TrayIconBuilder, TrayIconEvent,
};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use std::sync::{Arc, Mutex};
use tao::event::Event;

use crate::inputs;
use crate::settings;
use crate::touch_menu;

const MENU_SETTINGS: u32 = 1;
const MENU_EXIT: u32 = 2;
const MENU_TOGGLE: u32 = 3;

fn tooltip(enabled: bool) -> &'static str {
    if enabled { "Controller Tray (enabled)" } else { "Controller Tray (disabled)" }
}

#[derive(Debug, Clone)]
pub enum TrayEvent {
    ReloadSettings,
    ShowMenu { x: i32, y: i32 },
}

// Embedded so the exe works regardless of the working directory it's launched from.
const ICON_PNG: &[u8] = include_bytes!("../assets/game-controller.png");

pub fn load_icon_from_png(bytes: &[u8]) -> Result<Icon> {
    let img = image::load_from_memory(bytes).context("Failed to decode icon png")?.into_rgba8();
    let (w, h) = img.dimensions();
    let rgba = img.into_raw();
    Ok(Icon::from_rgba(rgba, w, h).context("Icon::from_rgba failed")?)
}

pub fn run_tray() -> Result<()>{
    let initial =  Arc::new(Mutex::new(settings::load_settings()?));

    inputs::spawn_polling_thread(initial.clone());

    // No native menu attached: the tray's default menu items are too small
    // for touch, so a large owner-drawn popup is shown on click instead.
    let icon = load_icon_from_png(ICON_PNG)?;
    let tray = TrayIconBuilder::new()
        .with_tooltip(tooltip(initial.lock().unwrap().enabled))
        .with_icon(icon)
        .build()
        .context("failed to create tray icon")?;

    // let event_loop = EventLoopBuilder::new().build();
    let event_loop = EventLoopBuilder::<TrayEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // Forward tray clicks (tap or right-click / long-press) through the proxy
    // so the event loop wakes up; polling a channel would wait for an
    // unrelated event before noticing the click.
    let click_proxy = Mutex::new(proxy.clone());
    TrayIconEvent::set_event_handler(Some(move |tray_event| {
        if let TrayIconEvent::Click { position, button_state: MouseButtonState::Up, .. } = tray_event {
            let _ = click_proxy.lock().unwrap().send_event(TrayEvent::ShowMenu {
                x: position.x as i32,
                y: position.y as i32,
            });
        }
    }));

    event_loop.run(move |event,_target, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(TrayEvent::ReloadSettings) => {
                if let Ok(new_s) = settings::load_settings() {
                    let _ = tray.set_tooltip(Some(tooltip(new_s.enabled)));
                    *initial.lock().unwrap() = new_s;
                }
            }
            Event::UserEvent(TrayEvent::ShowMenu { x, y }) => {
                let (large, enabled) = {
                    let s = initial.lock().unwrap();
                    (s.large_ui, s.enabled)
                };
                let toggle_label = if enabled { "Disable" } else { "Enable" };
                let choice = touch_menu::show(
                    &[
                        (MENU_TOGGLE, toggle_label),
                        (MENU_SETTINGS, "Settings..."),
                        (MENU_EXIT, "Exit"),
                    ],
                    x,
                    y,
                    large,
                );
                match choice {
                    Some(MENU_EXIT) => *control_flow = ControlFlow::Exit,
                    Some(MENU_TOGGLE) => {
                        let mut s = initial.lock().unwrap();
                        s.enabled = !s.enabled;
                        let _ = settings::save_settings(&s);
                        let _ = tray.set_tooltip(Some(tooltip(s.enabled)));
                    }
                    Some(MENU_SETTINGS) => {
                        let _ = settings::spawn_settings_window(proxy.clone());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    });

    Ok(())
}
