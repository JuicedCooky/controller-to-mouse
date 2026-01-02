use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT, MOUSEEVENTF_MOVE};
use windows::Win32::UI::Input::XboxController::{
    XInputGetState, XINPUT_STATE, XINPUT_GAMEPAD,
};
use std::{thread, time::{Duration, Instant}};

use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};

use crate::settings;

fn send_mouse_delta(dx: i32, dy: i32) {
    if dx == 0 && dy == 0 { return; }

    unsafe {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE,
                    time: 0,
                    dwExtraInfo: 0,
                }
            }
        };
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

fn read_xinput(user_index: u32) -> Option<XINPUT_GAMEPAD> {
    unsafe {
        let mut state = XINPUT_STATE::default();
        let res = XInputGetState(user_index, &mut state);
        if res == 0 {
            Some(state.Gamepad)
        } else {
            None
        }
    }
}

fn find_first_controller() -> Option<u32> {
    for i in 0..4 {
        if read_xinput(i).is_some() {
            return Some(i);
        }
    }
    None
}

pub fn spawn_polling_thread(settings: Arc<Mutex<settings::Settings>>) {
    std::thread::spawn(move || {    
        let idx = loop {
            if let Some(i) = find_first_controller() {
                break i;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        };


        println!("Connected controller at index {}", idx);
        // Here we just pretend dx/dy come from a stick mapping.

        let mut last = Instant::now();
        loop {
            let temp_settings = {settings.lock().unwrap().clone()};

            if let Some(pad) = read_xinput(idx) {
                let mut x = (pad.sThumbLX as f32) ;
                let mut y = (pad.sThumbLY as f32) ;

                if temp_settings.invert_y{y = -y};


                // TODO: deadzone + curve + sensitivity

                let now = Instant::now();
                let dt = now.duration_since(last);
                last = now;

                let dx = (x * temp_settings.sensitivity * dt.as_secs_f32()) as i32;
                let dy = (y * temp_settings.sensitivity * dt.as_secs_f32()) as i32;

                send_mouse_delta(dx, dy);

                thread::sleep(Duration::from_millis(1));
                println!("sens:{}",temp_settings.sensitivity);
                println!("x:{}\ny:{}", dx, dy);
            }
        }
    });
}