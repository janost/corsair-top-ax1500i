# corsair-top

Real-time TUI monitor for Corsair AX1600i power supplies.

![corsair-top](logo/corsairtop.png)

![screenshot](images/screenshot.png)

## Features

- Live input voltage, current, and power readings
- Per-rail breakdown (12V, 5V, 3.3V)
- 12V page-level current and OCP limit monitoring
- Temperature and fan speed display
- Per-PSU power and temperature sparkline graphs
- Combined total power chart with reference lines (1600W/1500W/1000W scaled by PSU count)
- Multi-PSU support with aligned panel layout
- Adjustable polling rate

## Requirements

- Linux
- Rust toolchain (1.87+)
- `libusb-1.0-0-dev`
- `pkg-config`
- Corsair AX1600i PSU connected via USB

## Install

```bash
git clone https://github.com/rgilbreth/corsair-top.git
cd corsair-top
./install.sh
```

This will:
1. Build the project in release mode
2. Install the binary to `/usr/local/bin/corsair-top`
3. Install a launch script with splash screen
4. Install a `.desktop` launcher with icon
5. Add a shell alias
6. Set up a udev rule for USB access

## Usage

```bash
corsair-top        # launch with splash (via alias)
sudo corsair-top   # run directly
```

### Controls

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit |
| `+` / `=` | Increase polling rate (min 250ms) |
| `-` | Decrease polling rate (max 5s) |

## USB Permissions

The install script creates a udev rule allowing members of the `plugdev` group to access the PSU without sudo:

```bash
sudo usermod -aG plugdev $USER
```

Log out and back in for the group change to take effect.

## License

MIT
