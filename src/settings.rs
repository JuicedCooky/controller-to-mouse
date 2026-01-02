use std::{
    fs, 
    path::{Path, PathBuf},
    env,
    process::Command,
};
use directories::ProjectDirs;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tao::event_loop::EventLoopProxy;

use eframe::egui;
use crate::tray::TrayEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub enabled: bool,
    pub invert_y: bool,
    pub sensitivity: f32,
    pub deadzone: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            invert_y: true,
            sensitivity: 1.0,
            deadzone: 0.075,
        }
    }
}

fn config_path() -> Result<PathBuf> {
    let proj = ProjectDirs::from("com", "AlanZ", "ControllerTray")
        .context("Failed to get ProjectDirs")?;
    let dir = proj.config_dir();
    fs::create_dir_all(dir).ok();
    Ok(dir.join("config.toml"))
}

pub fn load_settings() -> Result<Settings> {
    let path = config_path()?;
    if !path.exists() {
        let s = Settings::default();
        save_settings(&s)?;
        return Ok(s);
    }
    let txt = fs::read_to_string(&path).context("Reading config.toml")?;
    let s: Settings = toml::from_str(&txt).context("Parsing config.toml")?;
    Ok(s)
}

fn save_settings(s: &Settings) -> Result<()> {
    let path = config_path()?;
    let txt = toml::to_string_pretty(s).context("Serializing config")?;
    fs::write(&path, txt).context("Writing config.toml")?;
    Ok(())
}

fn exe_path() -> Result<PathBuf> {
    Ok(env::current_exe().context("current_exe failed")?)
}

pub fn spawn_settings_window(proxy: EventLoopProxy<TrayEvent>) -> anyhow::Result<()> {
    let exe = exe_path()?;
    let mut child = Command::new(exe)
        .arg("--settings")
        .spawn()
        .context("failed to spawn settings window")?;

    std::thread::spawn(move || {
        let _ = child.wait(); // blocks until settings window closes
        let _ = proxy.send_event(TrayEvent::ReloadSettings);
    });
    Ok(())
}

pub fn run_settings_window() -> Result<()> {
    let mut settings = load_settings().unwrap_or_default();
    
    struct App {
        settings: Settings,
        saved: bool,
    }

    impl eframe::App for App {
        fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
            use eframe::egui;

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Controller Tray Settings");
                ui.separator();

                ui.checkbox(&mut self.settings.enabled, "Enabled");
                ui.checkbox(&mut self.settings.invert_y, "Invert Y");

                ui.add(egui::Slider::new(&mut self.settings.sensitivity, 0.1..=2.0)
                    .text("Sensitivity"));
                ui.add(egui::Slider::new(&mut self.settings.deadzone, 0.0..=0.5)
                    .text("Deadzone"));

                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        let _ = save_settings(&self.settings);
                        self.saved = true;
                    }
                    if ui.button("Save & Close").clicked() {
                        let _ = save_settings(&self.settings);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("Close (No Save)").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);

                    }
                });

                if self.saved {
                    ui.label("Saved.");
                }
            });
        }
    }

    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([420.0, 260.0])
            .with_title("Settings"),
        ..Default::default()
    };

    eframe::run_native(
        "Settings",
        opts,
        Box::new(|_cc| Box::new(App { settings, saved: false })),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}