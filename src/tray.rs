use anyhow::{Context, Result};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, TrayIconBuilder,
};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use std::sync::{Arc, Mutex};
use tao::event::Event;

use crate::inputs;
use crate::settings;

#[derive(Debug, Clone)]
pub enum TrayEvent {
    ReloadSettings,
}

pub fn load_icon_from_png(path: &str) -> Result<Icon> {
    let img = image::open(path).context("Failed to open icon.png")?.into_rgba8();
    let (w, h) = img.dimensions();
    let rgba = img.into_raw();
    Ok(Icon::from_rgba(rgba, w, h).context("Icon::from_rgba failed")?)
}

pub fn run_tray() -> Result<()>{
    let initial =  Arc::new(Mutex::new(settings::load_settings()?));

    inputs::spawn_polling_thread(initial.clone());
 
    let menu = Menu::new();
    let exit_item = MenuItem::new("Exit", true, None);
    let settings_item = MenuItem::new("Settings...", true, None);
    // let reload_item = MenuItem::new("Reload settings", true, None);

    menu.append(&settings_item)?;
    menu.append(&exit_item)?;
    // menu.append(&reload_item)?;
    
    let icon = load_icon_from_png("assets/game-controller.png")?;
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Controller Tray")
        .with_icon(icon)
        .build();

    // let event_loop = EventLoopBuilder::new().build();
    let event_loop = EventLoopBuilder::<TrayEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    
    let menu_channel = MenuEvent::receiver();

    event_loop.run(move |event,_target, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(TrayEvent::ReloadSettings) => {
                if let Ok(new_s) = settings::load_settings() {
                    *initial.lock().unwrap() = new_s;
                    // optionally update UI text here (toggle labels, etc.)
                }
            }
            _ => {}
        }

        // Handle tray menu clicks
        if let Ok(menu_event) = menu_channel.try_recv() {
            if menu_event.id == exit_item.id() {
                *control_flow = ControlFlow::Exit;
            }
            else if menu_event.id == settings_item.id() {
                let _ = settings::spawn_settings_window(proxy.clone());
            }
            // else if menu_event.id == reload_item.id() {
            //     if let Ok(new_s) = settings::load_settings() {
            //         *initial.lock().unwrap() = new_s;
            //     }
            // }
        }
    });

    Ok(())
}