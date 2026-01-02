use std::{
    fs, 
    path::{PathBuf},
    env,
    process::Command,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
                ui.label(format!("available_width: {:.1}", ui.available_width()));

                if self.saved {
                    ui.label("Saved.");
                }
            });
        }
    }

    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            // .with_inner_size([420.0, 330.0])
            // .with_min_inner_size([320.0, 280.0])
            .with_resizable(true)
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