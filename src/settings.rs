use std::{
    fs, 
    path::{PathBuf},
    env,
    process::Command,
    time::{Duration, SystemTime},
};
use directories::ProjectDirs;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tao::event_loop::EventLoopProxy;

use eframe::egui;
use crate::tray::TrayEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum InputType {
    #[default]
    XInput,
    DirectInputSingle,
    DirectInputDual,
}

impl InputType {
    pub fn label(&self) -> &'static str {
        match self {
            InputType::XInput => "XInput (Xbox)",
            InputType::DirectInputSingle => "DirectInput (Single)",
            InputType::DirectInputDual => "DirectInput (Dual)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DualStickPriority {
    #[default]
    Stick1First,
    Stick2First,
    LargestMagnitude,
    CombineAdditive,
}

impl DualStickPriority {
    pub fn label(&self) -> &'static str {
        match self {
            DualStickPriority::Stick1First => "Stick 1 Priority",
            DualStickPriority::Stick2First => "Stick 2 Priority",
            DualStickPriority::LargestMagnitude => "Largest Movement",
            DualStickPriority::CombineAdditive => "Combine (Additive)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub enabled: bool,
    pub invert_y: bool,
    #[serde(default)]
    pub invert_x: bool,
    #[serde(default)]
    pub swap_axes: bool,
    pub sensitivity: f32,
    pub deadzone: f32,
    #[serde(default)]
    pub input_type: InputType,
    #[serde(default)]
    pub dual_stick_priority: DualStickPriority,
    #[serde(default = "default_true")]
    pub large_ui: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            invert_y: true,
            invert_x: false,
            swap_axes: false,
            sensitivity: 1.0,
            deadzone: 0.075,
            input_type: InputType::XInput,
            dual_stick_priority: DualStickPriority::Stick1First,
            large_ui: true,
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

pub fn save_settings(s: &Settings) -> Result<()> {
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

    // Reload whenever the settings window writes the config (every change is
    // applied live), not only when it closes.
    std::thread::spawn(move || {
        let mut last = config_modified();
        loop {
            let exited = !matches!(child.try_wait(), Ok(None));
            let now = config_modified();
            if now != last {
                last = now;
                let _ = proxy.send_event(TrayEvent::ReloadSettings);
            }
            if exited {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    });
    Ok(())
}

fn config_modified() -> Option<SystemTime> {
    fs::metadata(config_path().ok()?).ok()?.modified().ok()
}

const NORMAL_WINDOW_SIZE: [f32; 2] = [460.0, 340.0];
const LARGE_WINDOW_SIZE: [f32; 2] = [720.0, 560.0];

fn window_size(large: bool) -> [f32; 2] {
    if large { LARGE_WINDOW_SIZE } else { NORMAL_WINDOW_SIZE }
}

fn apply_ui_size(ctx: &egui::Context, large: bool) {
    ctx.set_zoom_factor(if large { 1.5 } else { 1.0 });
    ctx.style_mut(|style| {
        if large {
            style.spacing.button_padding = egui::vec2(12.0, 8.0);
            style.spacing.item_spacing = egui::vec2(10.0, 10.0);
            style.spacing.interact_size.y = 28.0;
        } else {
            let default = egui::Style::default();
            style.spacing.button_padding = default.spacing.button_padding;
            style.spacing.item_spacing = default.spacing.item_spacing;
            style.spacing.interact_size = default.spacing.interact_size;
        }
    });
}

pub fn run_settings_window() -> Result<()> {
    let settings = load_settings().unwrap_or_default();

    struct App {
        settings: Settings,
        // Settings when the window opened; restored on Cancel.
        original: Settings,
        // Last value written to disk, to save only when something changed.
        last_saved: Settings,
        confirmed: bool,
        // UI size currently applied to the window (None until first frame).
        shown_large: Option<bool>,
    }

    impl App {
        fn revert(&mut self) {
            self.settings = self.original.clone();
            if self.last_saved != self.original {
                let _ = save_settings(&self.original);
                self.last_saved = self.original.clone();
            }
        }
    }

    impl eframe::App for App {
        fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
            use eframe::egui;

            // Closing the window with the title-bar X counts as Cancel.
            if ctx.input(|i| i.viewport().close_requested()) && !self.confirmed {
                self.revert();
            }

            if self.shown_large != Some(self.settings.large_ui) {
                apply_ui_size(ctx, self.settings.large_ui);
                if self.shown_large.is_some() {
                    let [w, h] = window_size(self.settings.large_ui);
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
                }
                self.shown_large = Some(self.settings.large_ui);
            }

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Controller Tray Settings");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.settings.enabled, "Enabled");
                    ui.checkbox(&mut self.settings.large_ui, "Large (touch) UI");
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.settings.invert_x, "Invert X");
                    ui.checkbox(&mut self.settings.invert_y, "Invert Y");
                    ui.checkbox(&mut self.settings.swap_axes, "Swap X/Y");
                });

                ui.horizontal(|ui| {
                    ui.label("Input Type:");
                    egui::ComboBox::from_id_source("input_type")
                        .selected_text(self.settings.input_type.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.settings.input_type, InputType::XInput, InputType::XInput.label());
                            ui.selectable_value(&mut self.settings.input_type, InputType::DirectInputSingle, InputType::DirectInputSingle.label());
                            ui.selectable_value(&mut self.settings.input_type, InputType::DirectInputDual, InputType::DirectInputDual.label());
                        });
                });

                if self.settings.input_type == InputType::DirectInputDual {
                    ui.horizontal(|ui| {
                        ui.label("Dual Stick Priority:");
                        egui::ComboBox::from_id_source("dual_priority")
                            .selected_text(self.settings.dual_stick_priority.label())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.settings.dual_stick_priority, DualStickPriority::Stick1First, DualStickPriority::Stick1First.label());
                                ui.selectable_value(&mut self.settings.dual_stick_priority, DualStickPriority::Stick2First, DualStickPriority::Stick2First.label());
                                ui.selectable_value(&mut self.settings.dual_stick_priority, DualStickPriority::LargestMagnitude, DualStickPriority::LargestMagnitude.label());
                                ui.selectable_value(&mut self.settings.dual_stick_priority, DualStickPriority::CombineAdditive, DualStickPriority::CombineAdditive.label());
                            });
                    });
                }

                let label_width = 100.0;
                let value_width = 60.0;
                let row_h = 20.0;

                ui.horizontal(|ui| {
                    ui.add_sized([label_width, row_h], egui::Label::new("Sensitivity:"));
    
                    let w = (ui.available_width() - value_width).max(80.0);
                    ui.spacing_mut().slider_width = w;
                    
                    ui.add(
                        egui::Slider::new(&mut self.settings.sensitivity, 0.01..=2.0).show_value(false),
                    );
                
                    ui.add_sized([value_width, row_h], egui::Label::new(format!("{:.2}", self.settings.sensitivity)));
                });

                ui.horizontal(|ui| {
                    ui.add_sized([label_width, row_h], egui::Label::new("Deadzone:"));
    
                    let w = (ui.available_width() - value_width).max(80.0);
                    ui.spacing_mut().slider_width = w;
                    
                    ui.add(
                        egui::Slider::new(&mut self.settings.deadzone, 0.0..=0.5).show_value(false),
                    );
                
                    ui.add_sized([value_width, row_h], egui::Label::new(format!("{:.2}", self.settings.deadzone)));
                });


                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Confirm").clicked() {
                        self.confirmed = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("Cancel").clicked() {
                        self.revert();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    // Same as Cancel, but keeps the window open.
                    if ui.button("Reset").clicked() {
                        self.revert();
                    }
                });
            });

            // Apply changes live: write the config whenever a value changed.
            if self.settings != self.last_saved && !ctx.input(|i| i.viewport().close_requested()) {
                let _ = save_settings(&self.settings);
                self.last_saved = self.settings.clone();
            }
        }
    }

    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(window_size(settings.large_ui))
            .with_resizable(true)
            .with_title("Settings"),
        ..Default::default()
    };

    eframe::run_native(
        "Settings",
        opts,
        Box::new(|_cc| {
            Box::new(App {
                original: settings.clone(),
                last_saved: settings.clone(),
                settings,
                confirmed: false,
                shown_large: None,
            })
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}