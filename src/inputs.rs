use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT, MOUSEEVENTF_MOVE};
use windows::Win32::UI::Input::XboxController::{
    XInputGetState, XINPUT_STATE, XINPUT_GAMEPAD,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    DirectInput8Create, IDirectInput8W, IDirectInputDevice8W,
    DIDEVICEINSTANCEW, DIDATAFORMAT, DIOBJECTDATAFORMAT,
    DISCL_BACKGROUND, DISCL_NONEXCLUSIVE,
    DI8DEVCLASS_GAMECTRL, DIEDFL_ATTACHEDONLY,
    DIDF_ABSAXIS, DIPROP_RANGE, DIPROPRANGE, DIPROPHEADER,
    DIPH_BYOFFSET,
};
use windows::Win32::Foundation::{HINSTANCE, BOOL};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::core::{GUID, Interface};
use std::{thread, time::{Duration, Instant}};
use std::sync::{Arc, Mutex};
use std::cell::RefCell;
use std::mem;

use crate::settings::{self, InputType, DualStickPriority};

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

const DIRECTINPUT_VERSION: u32 = 0x0800;

// Standard DirectInput axis GUIDs
static GUID_XAXIS: GUID = GUID { data1: 0xA36D02E0, data2: 0xC9F3, data3: 0x11CF, data4: [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00] };
static GUID_YAXIS: GUID = GUID { data1: 0xA36D02E1, data2: 0xC9F3, data3: 0x11CF, data4: [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00] };
static GUID_ZAXIS: GUID = GUID { data1: 0xA36D02E2, data2: 0xC9F3, data3: 0x11CF, data4: [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00] };
static GUID_RXAXIS: GUID = GUID { data1: 0xA36D02F4, data2: 0xC9F3, data3: 0x11CF, data4: [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00] };
static GUID_RYAXIS: GUID = GUID { data1: 0xA36D02F5, data2: 0xC9F3, data3: 0x11CF, data4: [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00] };
static GUID_RZAXIS: GUID = GUID { data1: 0xA36D02E3, data2: 0xC9F3, data3: 0x11CF, data4: [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00] };
static GUID_SLIDER: GUID = GUID { data1: 0xA36D02E4, data2: 0xC9F3, data3: 0x11CF, data4: [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00] };
static GUID_POV: GUID = GUID { data1: 0xA36D02E5, data2: 0xC9F3, data3: 0x11CF, data4: [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00] };
static GUID_BUTTON: GUID = GUID { data1: 0xA36D02F0, data2: 0xC9F3, data3: 0x11CF, data4: [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00] };

// DIJOYSTATE structure - standard DirectInput joystick format (80 bytes)
#[repr(C)]
#[derive(Clone, Copy)]
struct DIJoyState {
    x: i32,      // 0
    y: i32,      // 4
    z: i32,      // 8
    rx: i32,     // 12
    ry: i32,     // 16
    rz: i32,     // 20
    slider: [i32; 2], // 24, 28
    pov: [u32; 4],    // 32, 36, 40, 44
    buttons: [u8; 32], // 48-79
}

impl Default for DIJoyState {
    fn default() -> Self {
        Self {
            x: 32767, y: 32767, z: 32767,  // Center values
            rx: 32767, ry: 32767, rz: 32767,
            slider: [32767; 2],
            pov: [0xFFFFFFFF; 4], // -1 = centered
            buttons: [0; 32],
        }
    }
}

// Build c_dfDIJoystick equivalent data format with proper GUIDs
fn create_joystick_data_format() -> (DIDATAFORMAT, Vec<DIOBJECTDATAFORMAT>) {
    // DIDFT constants
    const DIDFT_ABSAXIS: u32 = 0x00000002;
    const DIDFT_POV: u32 = 0x00000010;
    const DIDFT_PSHBUTTON: u32 = 0x00000004;
    const DIDFT_OPTIONAL: u32 = 0x80000000;
    const DIDFT_ANYINSTANCE: u32 = 0x00FFFF00;

    let mut objects = Vec::new();

    // X axis at offset 0
    objects.push(DIOBJECTDATAFORMAT {
        pguid: &GUID_XAXIS as *const GUID,
        dwOfs: 0,
        dwType: DIDFT_OPTIONAL | DIDFT_ABSAXIS | DIDFT_ANYINSTANCE,
        dwFlags: 0,
    });
    // Y axis at offset 4
    objects.push(DIOBJECTDATAFORMAT {
        pguid: &GUID_YAXIS as *const GUID,
        dwOfs: 4,
        dwType: DIDFT_OPTIONAL | DIDFT_ABSAXIS | DIDFT_ANYINSTANCE,
        dwFlags: 0,
    });
    // Z axis at offset 8
    objects.push(DIOBJECTDATAFORMAT {
        pguid: &GUID_ZAXIS as *const GUID,
        dwOfs: 8,
        dwType: DIDFT_OPTIONAL | DIDFT_ABSAXIS | DIDFT_ANYINSTANCE,
        dwFlags: 0,
    });
    // Rx axis at offset 12
    objects.push(DIOBJECTDATAFORMAT {
        pguid: &GUID_RXAXIS as *const GUID,
        dwOfs: 12,
        dwType: DIDFT_OPTIONAL | DIDFT_ABSAXIS | DIDFT_ANYINSTANCE,
        dwFlags: 0,
    });
    // Ry axis at offset 16
    objects.push(DIOBJECTDATAFORMAT {
        pguid: &GUID_RYAXIS as *const GUID,
        dwOfs: 16,
        dwType: DIDFT_OPTIONAL | DIDFT_ABSAXIS | DIDFT_ANYINSTANCE,
        dwFlags: 0,
    });
    // Rz axis at offset 20
    objects.push(DIOBJECTDATAFORMAT {
        pguid: &GUID_RZAXIS as *const GUID,
        dwOfs: 20,
        dwType: DIDFT_OPTIONAL | DIDFT_ABSAXIS | DIDFT_ANYINSTANCE,
        dwFlags: 0,
    });
    // 2 sliders at offsets 24, 28
    for i in 0..2u32 {
        objects.push(DIOBJECTDATAFORMAT {
            pguid: &GUID_SLIDER as *const GUID,
            dwOfs: 24 + i * 4,
            dwType: DIDFT_OPTIONAL | DIDFT_ABSAXIS | DIDFT_ANYINSTANCE,
            dwFlags: 0,
        });
    }
    // 4 POVs at offsets 32, 36, 40, 44
    for i in 0..4u32 {
        objects.push(DIOBJECTDATAFORMAT {
            pguid: &GUID_POV as *const GUID,
            dwOfs: 32 + i * 4,
            dwType: DIDFT_OPTIONAL | DIDFT_POV | DIDFT_ANYINSTANCE,
            dwFlags: 0,
        });
    }
    // 32 buttons at offsets 48-79
    for i in 0..32u32 {
        objects.push(DIOBJECTDATAFORMAT {
            pguid: &GUID_BUTTON as *const GUID,
            dwOfs: 48 + i,
            dwType: DIDFT_OPTIONAL | DIDFT_PSHBUTTON | DIDFT_ANYINSTANCE,
            dwFlags: 0,
        });
    }

    let format = DIDATAFORMAT {
        dwSize: mem::size_of::<DIDATAFORMAT>() as u32,
        dwObjSize: mem::size_of::<DIOBJECTDATAFORMAT>() as u32,
        dwFlags: DIDF_ABSAXIS,
        dwDataSize: mem::size_of::<DIJoyState>() as u32,
        dwNumObjs: objects.len() as u32,
        rgodf: std::ptr::null_mut(),
    };

    (format, objects)
}

struct DirectInputContext {
    dinput: IDirectInput8W,
    #[allow(dead_code)]
    devices: Vec<IDirectInputDevice8W>,
}

impl DirectInputContext {
    fn new() -> Option<Self> {
        unsafe {
            // Initialize COM
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            // Get module handle
            let hinst: HINSTANCE = GetModuleHandleW(None).ok()?.into();

            // Create DirectInput8 object
            let mut dinput: Option<IDirectInput8W> = None;
            let hr = DirectInput8Create(
                hinst,
                DIRECTINPUT_VERSION,
                &IDirectInput8W::IID,
                &mut dinput as *mut _ as *mut *mut std::ffi::c_void,
                None,
            );

            if hr.is_err() {
                println!("Failed to create DirectInput8: {:?}", hr);
                return None;
            }

            let dinput = dinput?;

            Some(DirectInputContext {
                dinput,
                devices: Vec::new(),
            })
        }
    }

    fn enumerate_devices(&mut self) -> Vec<GUID> {
        // We need to use a thread_local for the callback trampoline
        thread_local! {
            static GUIDS: RefCell<Vec<GUID>> = const { RefCell::new(Vec::new()) };
        }

        GUIDS.with(|g| g.borrow_mut().clear());

        unsafe {
            extern "system" fn enum_callback(
                device_instance: *mut DIDEVICEINSTANCEW,
                _context: *mut std::ffi::c_void,
            ) -> BOOL {
                unsafe {
                    if !device_instance.is_null() {
                        let instance = &*device_instance;
                        GUIDS.with(|g| g.borrow_mut().push(instance.guidInstance));
                    }
                }
                BOOL(1) // DIENUM_CONTINUE
            }

            let _ = self.dinput.EnumDevices(
                DI8DEVCLASS_GAMECTRL,
                Some(enum_callback),
                std::ptr::null_mut(),
                DIEDFL_ATTACHEDONLY,
            );
        }

        GUIDS.with(|g| g.borrow().clone())
    }

    fn create_device(&mut self, guid: &GUID) -> Option<IDirectInputDevice8W> {
        unsafe {
            let mut device: Option<IDirectInputDevice8W> = None;

            self.dinput
                .CreateDevice(
                    guid,
                    &mut device,
                    None, // pUnkOuter (almost always None)
                )
                .ok()?;
            
            let device = device?;

            // Set cooperative level (background + nonexclusive)
            device.SetCooperativeLevel(
                None, // HWND - None for background
                DISCL_BACKGROUND | DISCL_NONEXCLUSIVE,
            ).ok()?;

            // Set data format using c_dfDIJoystick equivalent
            let (mut format, mut objects) = create_joystick_data_format();
            format.rgodf = objects.as_mut_ptr();

            let fmt_result = device.SetDataFormat(&mut format as *mut DIDATAFORMAT);
            if let Err(e) = fmt_result {
                println!("SetDataFormat failed: {:?}", e);
                return None;
            }
            println!("SetDataFormat succeeded");

            // Set axis range to 0-65535 (standard DirectInput range)
            // We'll convert to signed in read_device
            for offset in [0u32, 4u32] {
                let mut prop_range = DIPROPRANGE {
                    diph: DIPROPHEADER {
                        dwSize: mem::size_of::<DIPROPRANGE>() as u32,
                        dwHeaderSize: mem::size_of::<DIPROPHEADER>() as u32,
                        dwObj: offset,
                        dwHow: DIPH_BYOFFSET,
                    },
                    lMin: 0,
                    lMax: 65535,
                };
                let _ = device.SetProperty(
                    &DIPROP_RANGE,
                    &mut prop_range.diph as *mut DIPROPHEADER,
                );
            }

            // Acquire the device
            device.Acquire().ok()?;

            Some(device)
        }
    }

    fn read_device(device: &IDirectInputDevice8W) -> Option<(i32, i32)> {
        unsafe {
            // Poll the device first
            let _ = device.Poll();

            // Read joystick state
            let mut state = DIJoyState::default();
            let result = device.GetDeviceState(
                mem::size_of::<DIJoyState>() as u32,
                &mut state as *mut _ as *mut std::ffi::c_void,
            );

            if result.is_ok() {
                // Debug: print raw values
                println!("raw: x={} y={} z={} rx={} ry={} rz={}",
                    state.x, state.y, state.z, state.rx, state.ry, state.rz);

                // Convert from DirectInput range (typically 0-65535 with center at 32767)
                // to signed range (-32768 to 32767 with center at 0)
                let x = state.x - 32767;
                let y = state.y - 32767;
                Some((x, y))
            } else {
                println!("GetDeviceState failed: {:?}", result);
                // Try to reacquire if we lost the device
                let _ = device.Acquire();
                None
            }
        }
    }
}

fn apply_deadzone(value: f32, deadzone: f32) -> f32 {
    let max_val = 32767.0;
    let deadzone_threshold = deadzone * max_val;

    if value.abs() < deadzone_threshold {
        0.0
    } else {
        // Scale the remaining range
        let sign = value.signum();
        let abs_val = value.abs();
        sign * ((abs_val - deadzone_threshold) / (max_val - deadzone_threshold)) * max_val
    }
}

pub fn spawn_polling_thread(settings: Arc<Mutex<settings::Settings>>) {
    std::thread::spawn(move || {
        let mut last = Instant::now();
        let mut dinput_ctx: Option<DirectInputContext> = None;
        let mut dinput_devices: Vec<IDirectInputDevice8W> = Vec::new();
        let mut xinput_idx: Option<u32> = None;
        let mut current_input_type: Option<InputType> = None;

        loop {
            let temp_settings = { settings.lock().unwrap().clone() };

            if !temp_settings.enabled {
                thread::sleep(Duration::from_millis(100));
                last = Instant::now();
                continue;
            }

            // Check if input type changed - reinitialize if needed
            if current_input_type != Some(temp_settings.input_type) {
                current_input_type = Some(temp_settings.input_type);
                dinput_devices.clear();
                dinput_ctx = None;
                xinput_idx = None;
                println!("Switching to input type: {:?}", temp_settings.input_type);
            }

            let (x, y) = match temp_settings.input_type {
                InputType::XInput => {
                    // Initialize XInput if needed
                    if xinput_idx.is_none() {
                        xinput_idx = find_first_controller();
                        if xinput_idx.is_some() {
                            println!("XInput: Connected controller at index {}", xinput_idx.unwrap());
                        }
                    }

                    if let Some(idx) = xinput_idx {
                        if let Some(pad) = read_xinput(idx) {
                            (pad.sThumbLX as f32, pad.sThumbLY as f32)
                        } else {
                            // Lost controller, try to find again
                            xinput_idx = None;
                            thread::sleep(Duration::from_millis(100));
                            continue;
                        }
                    } else {
                        thread::sleep(Duration::from_millis(250));
                        continue;
                    }
                }

                InputType::DirectInputSingle => {
                    // Initialize DirectInput if needed
                    if dinput_ctx.is_none() {
                        dinput_ctx = DirectInputContext::new();
                    }

                    if let Some(ref mut ctx) = dinput_ctx {
                        // Enumerate and create device if needed
                        if dinput_devices.is_empty() {
                            let guids = ctx.enumerate_devices();
                            if let Some(guid) = guids.first() {
                                if let Some(device) = ctx.create_device(guid) {
                                    println!("DirectInput: Connected single joystick");
                                    dinput_devices.push(device);
                                }
                            }
                        }

                        if let Some(device) = dinput_devices.first() {
                            if let Some((x, y)) = DirectInputContext::read_device(device) {
                                (x as f32, y as f32)
                            } else {
                                thread::sleep(Duration::from_millis(100));
                                continue;
                            }
                        } else {
                            thread::sleep(Duration::from_millis(250));
                            continue;
                        }
                    } else {
                        println!("Failed to initialize DirectInput");
                        thread::sleep(Duration::from_millis(1000));
                        continue;
                    }
                }

                InputType::DirectInputDual => {
                    // Initialize DirectInput if needed
                    if dinput_ctx.is_none() {
                        dinput_ctx = DirectInputContext::new();
                    }

                    if let Some(ref mut ctx) = dinput_ctx {
                        // Enumerate and create devices if needed
                        if dinput_devices.len() < 2 {
                            dinput_devices.clear();
                            let guids = ctx.enumerate_devices();

                            for guid in guids.iter().take(2) {
                                if let Some(device) = ctx.create_device(guid) {
                                    dinput_devices.push(device);
                                }
                            }

                            if dinput_devices.len() >= 2 {
                                println!("DirectInput: Connected dual joysticks");
                            } else if dinput_devices.len() == 1 {
                                println!("DirectInput: Only 1 joystick found, using single mode");
                            }
                        }

                        match dinput_devices.len() {
                            2 => {
                                // Read both joysticks (both control full X/Y)
                                let stick1 = DirectInputContext::read_device(&dinput_devices[0])
                                    .map(|(x, y)| (x as f32, y as f32))
                                    .unwrap_or((0.0, 0.0));
                                let stick2 = DirectInputContext::read_device(&dinput_devices[1])
                                    .map(|(x, y)| (x as f32, y as f32))
                                    .unwrap_or((0.0, 0.0));

                                // Apply priority logic
                                let deadzone_threshold = temp_settings.deadzone * 32767.0;
                                let stick1_active = stick1.0.abs() > deadzone_threshold || stick1.1.abs() > deadzone_threshold;
                                let stick2_active = stick2.0.abs() > deadzone_threshold || stick2.1.abs() > deadzone_threshold;

                                match temp_settings.dual_stick_priority {
                                    DualStickPriority::Stick1First => {
                                        if stick1_active { stick1 } else { stick2 }
                                    }
                                    DualStickPriority::Stick2First => {
                                        if stick2_active { stick2 } else { stick1 }
                                    }
                                    DualStickPriority::LargestMagnitude => {
                                        let mag1 = stick1.0 * stick1.0 + stick1.1 * stick1.1;
                                        let mag2 = stick2.0 * stick2.0 + stick2.1 * stick2.1;
                                        if mag1 >= mag2 { stick1 } else { stick2 }
                                    }
                                    DualStickPriority::CombineAdditive => {
                                        (stick1.0 + stick2.0, stick1.1 + stick2.1)
                                    }
                                }
                            }
                            1 => {
                                // Fallback to single joystick
                                if let Some((x, y)) = DirectInputContext::read_device(&dinput_devices[0]) {
                                    (x as f32, y as f32)
                                } else {
                                    thread::sleep(Duration::from_millis(100));
                                    continue;
                                }
                            }
                            _ => {
                                thread::sleep(Duration::from_millis(250));
                                continue;
                            }
                        }
                    } else {
                        println!("Failed to initialize DirectInput");
                        thread::sleep(Duration::from_millis(1000));
                        continue;
                    }
                }
            };
            
            println!("x:{} y:{}",x,y);

            // Apply deadzone
            let x = apply_deadzone(x, temp_settings.deadzone);
            let mut y = apply_deadzone(y, temp_settings.deadzone);

            if temp_settings.invert_y {
                y = -y;
            }

            let now = Instant::now();
            let dt = now.duration_since(last);
            last = now;

            let dx = (x * temp_settings.sensitivity * dt.as_secs_f32()) as i32;
            let dy = (y * temp_settings.sensitivity * dt.as_secs_f32()) as i32;

            println!("dx:{} dy:{}",dx,dy);

            send_mouse_delta(dx, dy);

            thread::sleep(Duration::from_millis(1));
        }
    });
}