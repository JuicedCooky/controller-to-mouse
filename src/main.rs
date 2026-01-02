// #![windows_subsystem = "windows"]

use anyhow::{Context, Result};
use std::{env};



mod tray;
mod settings;
mod inputs;




fn main() -> Result<()>{
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--settings"){
        settings::run_settings_window()
    }
    else{
        tray::run_tray()
    }
}