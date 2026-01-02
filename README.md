# Controller Tray

A Windows system tray application that maps game controller joystick input to mouse cursor movement. Supports both XInput (Xbox) and DirectInput devices, including configurations with dual detached controllers.

Originally created for the Lenovo Legion Go, where one or both controllers can be detached from the main unit.

## Features

- **XInput support** - Works with Xbox controllers and compatible devices
- **DirectInput support** - Works with generic game controllers
  - Single device mode
  - Dual device mode (two separate joysticks controlling the same cursor)
- **Dual stick priority modes** - When using two controllers:
  - Stick 1 Priority - Use first stick, fallback to second
  - Stick 2 Priority - Use second stick, fallback to first
  - Largest Movement - Use whichever stick has more deflection
  - Combine Additive - Sum both stick inputs together
- **Configurable sensitivity and deadzone**
- **Axis options** - Invert X, invert Y, or swap X/Y axes
- **System tray integration** - Runs quietly in the background
- **Persistent settings** - Configuration saved to `config.toml`

## Installation

1. Download `ControllerTray.zip` from the releases
⬇ **[Download ControllerTray (Windows)](https://github.com/JuicedCooky/ControllerTray/releases/latest)**

2. Extract the zip to a folder of your choice
3. Run `ControllerTray.exe`

The extracted folder structure should look like:
```
ControllerTray/
├── ControllerTray.exe
└── assets/
    └── game-controller.png
```

## Usage

1. Run `ControllerTray.exe` - it will appear in the system tray
2. Right-click the tray icon to access:
   - **Settings** - Open the configuration window
   - **Exit** - Close the application

### Settings

| Setting | Description |
|---------|-------------|
| Enabled | Toggle joystick-to-mouse mapping on/off |
| Invert X/Y | Reverse the axis direction |
| Swap X/Y | Exchange horizontal and vertical axes |
| Input Type | Choose between XInput, DirectInput Single, or DirectInput Dual |
| Dual Stick Priority | How to handle input when two controllers are connected |
| Sensitivity | Mouse movement speed multiplier (0.01 - 2.0) |
| Deadzone | Ignore small stick movements (0.0 - 0.5) |

### Command Line

```
ControllerTray.exe              # Run in tray mode (default)
ControllerTray.exe --settings   # Open settings window directly
```

## Configuration

Settings are stored in:
```
%APPDATA%\AlanZ\ControllerTray\config.toml
```

## Building from Source

Requires Rust. Run the build script:

```powershell
./build.ps1
```

This will:
1. Build the release binary
2. Package everything into `dist/ControllerTray.zip`
