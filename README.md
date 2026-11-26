# UXF-Control-Center

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](./LICENSE)
[![Platform: Fedora Linux](https://img.shields.io/badge/Platform-Fedora%2045-blue.svg)](https://fedoraproject.org/)
[![Architecture: x86_64](https://img.shields.io/badge/Arch-x86__64-brightgreen.svg)]()
[![Release: v2.0.2-2](https://img.shields.io/badge/Release-v2.0.2--2-orange.svg)](https://github.com/Urxfather/UXF-Control-Center/releases)

**UXF-Control-Center** is an independently maintained, modern Linux control center and DKMS hardware driver suite built specifically for supported Acer gaming laptops. 

It pairs a high-performance desktop interface (**Tauri v2 + React 19**) with a native system daemon (`acer-control-daemon`), low-level character device ioctl drivers (`acer_io`), and ACPI embedded-controller backends (`acer_acpi`) to bring full fan speed controls, thermal monitoring, RGB keyboard lighting effects, and desktop integration to Linux users.

---

## Hardware Compatibility

Different Acer laptop models use varied embedded controllers (EC), ACPI methods, and WMI interfaces. UXF-Control-Center features hardware-aware probing to ensure safe and stable operation.

### Tested & Verified Hardware
* **Acer Aspire 7 A715-79G** (`AlgAl15g53`, Intel Core 5 210H, NVIDIA RTX 3050 / Intel Arc)
  * **Backend:** `acer_acpi` (EC ACPI interface)
  * **Cooling:** Dual-channel fan tachometer telemetry and Auto / Max fan control.
  * **Lighting:** RGB static color, 4 brightness steps, Rainbow cycle, and Breathing mode.
  * **Telemetry:** Real-time CPU/GPU temperatures, live CPU frequencies, memory, and storage metrics.
  * **Desktop Controls:** Display brightness, GNOME Blue Light (Night Light), and system audio volume.

### Recognized Acer Models
Other Acer gaming laptops (such as select Nitro and Predator models) are recognized and supported through dynamic backend selection (`acer_acpi` or fallback `acer_wmi`). Some proprietary features may vary depending on firmware capabilities.

### Untested / Non-Acer Hardware
UXF-Control-Center safely checks DMI identifiers on startup. If an unverified or non-Acer motherboard is detected, the application operates in safe read-only mode to prevent unintended hardware mutations.

> [!NOTE]
> **Flexikey & FlexiCharger:** Key macro binding and battery charge threshold limiters are currently under architectural development and remain in a "Coming Soon" state.

---

## Features

- **Modern Desktop GUI:** Built with Tauri v2, React 19, and Vite for ultra-low memory overhead (~30 MB RAM) and sub-second launch times.
- **Cooling & Fan Controls:** Real-time CPU and GPU tachometers with one-click toggles between Auto and Max speed profiles.
- **RGB Keyboard Lighting:** Full control over static RGB lighting (custom hex/RGB wheel), 4-step backlight brightness, Rainbow gradient animation, and Breathing effects.
- **Hardware Telemetry Dashboard:** Live monitoring of CPU/GPU diode temperatures, clock speeds, system RAM, and swap/storage allocations.
- **Desktop Controls & Preferences:** Integrated screen brightness slider, GNOME Blue Light (Night Light) toggle with color temperature slider, system audio volume, touchpad toggle, camera switch, and lock key indicators.
- **Automated DKMS Driver:** Includes the `acer-keyboard 0.1.1` kernel driver with Linux kernel 7.3+ compatibility, automated multi-kernel builds, and udev permission rules.
- **First-Launch Health Diagnostics:** In-app subsystem diagnostic audit (accessible via the `[i]` icon) scanning hardware DMI, kernel modules, DKMS build status, Secure Boot MOK keys, D-Bus communication, and device permissions.
- **Automated Update Alerts:** Background GitHub Releases checker with in-app notifications and copyable DNF update commands.
- **Fallback Desktop Interface:** The original eframe/egui GUI is retained at `/usr/bin/acer-control-center` as a fallback.

---

## Screenshots & Application Preview

Every screenshot below is captured directly from the running UXF-Control-Center application on an Acer Aspire 7 A715-79G (`AlgAl15g53`) under Linux.

### Dashboard
The central control center view presents hardware identification, real-time CPU tachometer dial, fan status, memory/storage gauges, and desktop control sliders in an ultra-responsive dark interface.

![UXF-Control-Center Dashboard](docs/screenshots/dashboard.png)
*Figure 1: Full UXF-Control-Center main dashboard featuring live telemetry, tachometer dial, and system toggles.*

---

### Performance & Cooling
Monitor live thermal conditions, core clocks, and fan RPM speeds. Switch cooling profiles between Dynamic Auto and Maximum fan speed with instant D-Bus hardware response.

![CPU, GPU, and System Telemetry](docs/screenshots/system-telemetry.png)
*Figure 2: Real-time telemetry monitoring for Intel Core 5 210H, NVIDIA graphics, RAM allocation, and CPU frequency.*

![Fan Monitoring and Cooling Controls](docs/screenshots/fan-controls.png)
*Figure 3: Dual-channel fan tachometers (CPU & GPU Fan RPM) and Auto / Max cooling mode controls.*

---

### Keyboard & RGB Lighting
Customizable RGB keyboard backlight with 4-step brightness levels, interactive color wheel, hex color codes, and animated lighting modes.

![Keyboard RGB Static Colour Controls](docs/screenshots/keyboard-rgb-static.png)
*Figure 4: Single-zone RGB static lighting with 360° color wheel, preset color palettes, and real-time layout preview.*

![Rainbow Wave Lighting Mode](docs/screenshots/keyboard-rgb-rainbow.png)
*Figure 5: Rainbow Wave spectrum cycling mode.*

![Breathing Pulse Mode](docs/screenshots/keyboard-rgb-breathing.png)
*Figure 6: Breathing Pulse lighting mode.*

![Breathing Pulse Live Animation](docs/screenshots/keyboard-rgb-breathing.gif)
*Figure 7: Live animation demonstration of Breathing Pulse mode on the virtual keyboard preview.*

![Keyboard Brightness & Sleep Timer](docs/screenshots/keyboard-brightness.png)
*Figure 8: 4-step backlight brightness slider and GNOME Mutter idle-sleep timer configuration.*

---

### System Controls
Seamless desktop environment integrations that work alongside GNOME and systemd.

![Blue Light & Night Light Controls](docs/screenshots/blue-light.png)
*Figure 9: GNOME Night Light warmth slider and automated sunrise/sunset schedule integration.*

![System Volume Controls](docs/screenshots/volume-controls.png)
*Figure 10: Desktop audio volume slider with instant hardware mute toggle.*

---

### Preferences & Diagnostics
Hardware-aware preferences grid and built-in First-Launch Diagnostics to verify driver and subsystem health.

![Preferences Grid](docs/screenshots/preferences.png)
*Figure 11: System preferences grid with hardware and desktop toggles.*

![Super Lock & Gaming Toggles](docs/screenshots/super-lock.png)
*Figure 12: Super Lock (Windows key lock), Super-Fn swap, and Fn Lock gaming preferences.*

![System Health Check Diagnostics](docs/screenshots/health-check.png)
*Figure 13: Integrated Health Diagnostic tool verifying DMI hardware matching, DKMS driver compilation, loaded kernel modules, Secure Boot MOK keys, D-Bus daemon connectivity, and device permissions.*

![About Dialog](docs/screenshots/about.png)
*Figure 14: About dialog showing application identity, official release version `v2.0.2-2`, maintainer info, and GPL-3.0 licensing.*

![GitHub Update Checker](docs/screenshots/update-checker.png)
*Figure 15: Background GitHub Releases update checker reporting current version parity.*

---

### Coming Soon
Unfinished features are transparently displayed in their current development state with planned architectural roadmaps.

![Flexikey — Coming Soon](docs/screenshots/flexikey-coming-soon.png)
*Figure 16: Flexikey key-remapping and macro configuration section (Coming Soon — under architectural development).*

![FlexiCharger — Coming Soon](docs/screenshots/flexicharger-coming-soon.png)
*Figure 17: FlexiCharger battery charge threshold limiter section (Coming Soon — requires platform EC battery threshold support).*

---

## Installation (Fedora 45 / x86_64)

UXF-Control-Center is packaged as a production-grade RPM package that handles dependency resolution, DKMS driver compilation, udev permissions, and systemd service startup automatically.

### 1. Download the Latest RPM
Download `uxf-control-center-2.0.2-2.x86_64.rpm` from [GitHub Releases](https://github.com/Urxfather/UXF-Control-Center/releases).

### 2. Install via DNF
Open a terminal in the folder where the RPM was downloaded and run:

```bash
sudo dnf install ./uxf-control-center-2.0.2-2.x86_64.rpm
```

DNF will automatically resolve and install external dependencies (`dkms`, `kernel-devel-matched`, `libwebkit2gtk-4.1`, etc.), compile the kernel module for your running kernel, configure device permissions, and start the system daemon.

### 3. Launch
Launch **UXF-Control-Center** from your desktop application launcher or run:

```bash
uxf-control-center
```

---

## Secure Boot & MOK Enrollment

If Secure Boot is enabled in your UEFI firmware, kernel module signature verification is required:

1. During installation, DKMS automatically signs the compiled module using a local Machine Owner Key (`/var/lib/dkms/mok.pub`).
2. If your system requires key enrollment, run:
   ```bash
   sudo mokutil --import /var/lib/dkms/mok.pub
   ```
3. Enter a temporary one-time password, reboot your laptop, and select **"Enroll MOK"** in the blue UEFI MokManager screen.
4. Enter your temporary password and confirm enrollment.

---

## Building from Source

### Prerequisites
On Fedora Linux, install the required build tools and libraries:

```bash
sudo dnf install -y \
    gcc gcc-c++ make cmake meson ninja-build cargo rust nodejs npm \
    webkit2gtk4.1-devel gtk3-devel libsoup3-devel openssl-devel \
    dbus-devel libX11-devel libxkbcommon-devel wayland-devel mesa-libGL-devel \
    dkms kernel-devel-matched
```

### Build Instructions

1. **Clone the Repository:**
   ```bash
   git clone https://github.com/Urxfather/UXF-Control-Center.git
   cd UXF-Control-Center
   ```

2. **Build C Core Library (`libacercontrol`):**
   ```bash
   meson setup build/meson lib --prefix=/usr --libdir=lib64 --buildtype=release
   meson compile -C build/meson
   ```

3. **Build the Hardware Daemon:**
   ```bash
   cargo build --release --manifest-path daemon/Cargo.toml
   ```

4. **Build the Tauri v2 GUI:**
   ```bash
   cd gui-tauri
   npm install
   npm run build
   cargo build --release --manifest-path src-tauri/Cargo.toml
   cd ..
   ```

5. **Build RPM Package:**
   ```bash
   LIBDIR=/usr/lib64 packaging/build-package.sh rpm
   ```
   The built RPM package will be located in `dist/uxf-control-center-2.0.2-2.x86_64.rpm`.

---

## Troubleshooting

### Daemon Connection Failed (D-Bus)
Verify that `acer-control-daemon.service` is running:
```bash
systemctl status acer-control-daemon.service
```
Restart the service if needed:
```bash
sudo systemctl restart acer-control-daemon.service
```

### Fan Controls Unavailable (`/dev/acer_io` missing)
Check whether the kernel driver modules are loaded:
```bash
lsmod | grep acer
```
If modules are missing, load them manually:
```bash
sudo modprobe acer_keyboard acer_acpi acer_io
```

### Missing Kernel Headers
If your kernel was recently updated without development headers:
```bash
sudo dnf install kernel-devel-matched
sudo dkms autoinstall
```

---

## AI-Assisted Hardware Compatibility — Adapt UXF-Control-Center for Your Laptop

Because different Acer gaming laptops (Aspire 7, Nitro 5, Nitro 16, Predator Helios, Predator Triton) use different embedded controller (EC) registers, ACPI DCHU device nodes, or WMI GUIDs, you can use modern AI assistants (such as ChatGPT, Gemini, or Claude) to help adapt UXF-Control-Center for your hardware.

### Step 1: Collect Your Laptop's Hardware Diagnostics
Run the following safe, read-only diagnostic commands in your terminal:

```bash
echo "=== DMI System Info ==="
cat /sys/class/dmi/id/sys_vendor
cat /sys/class/dmi/id/product_name
cat /sys/class/dmi/id/board_name

echo "=== Kernel & OS ==="
uname -r
cat /etc/os-release | grep PRETTY_NAME

echo "=== Loaded Acer Modules ==="
lsmod | grep acer

echo "=== ACPI / EC Information ==="
ls -d /sys/devices/platform/acer* 2>/dev/null
ls -d /sys/class/leds/*kbd* 2>/dev/null

echo "=== Character Device Access ==="
ls -l /dev/acer_io 2>/dev/null
```

*(Optional elevated check to see EC memory access response)*:
```bash
sudo dmesg | grep -iE "acer|wmi|dchu"
```

### Step 2: Copy the AI Prompt Template
Copy and paste the template below into your AI coding assistant, attaching your diagnostic output from Step 1:

````markdown
I am running Linux on an Acer laptop and would like to adapt UXF-Control-Center (https://github.com/Urxfather/UXF-Control-Center) for my specific model.

Here are my hardware diagnostic details:
<PASTE YOUR DIAGNOSTIC COMMAND OUTPUT HERE>

Please inspect the UXF-Control-Center architecture:
1. Primary verified reference: Acer Aspire 7 A715-79G (AlgAl15g53) using `acer_acpi` (driver/src/backends/acer/acer_acpi.c) and `acer_io` (driver/src/acer_io/acer_io.c).
2. Daemon D-Bus interface: `io.github.stanlysilas.AcerControl` (daemon/src/backend.rs, daemon/src/dbus.rs).
3. Compare my model's ACPI device paths and DMI identifiers with the existing backends.
4. Identify which features (fan tachometers, fan speed toggles, RGB lighting, thermal diode telemetry) can realistically be supported on my machine.
5. Provide the exact minimal code diff or driver backend adaptations required, ensuring existing A715-79G support is not broken.
6. Outline safe, read-only testing steps to verify before building kernel modules.
````

### Step 3: Contribute Your Findings
AI-assisted modifications must be tested and reviewed before being merged into the official repository. Open an issue or pull request on [GitHub Issues](https://github.com/Urxfather/UXF-Control-Center/issues) sharing your laptop model, diagnostic logs, and proposed diffs so other Linux users with your hardware can benefit.

> [!WARNING]
> - AI assistance does not guarantee hardware compatibility.
> - Never issue unverified low-level EC write commands or blindly flash untested kernel modules.
> - Always maintain a bootable kernel backup when testing custom DKMS builds.

---

## Contributing & Community

Contributions, bug reports, and hardware compatibility reports are welcome!
- **Bug Reports & Feature Requests:** [GitHub Issues](https://github.com/Urxfather/UXF-Control-Center/issues)
- **Pull Requests:** [GitHub Pull Requests](https://github.com/Urxfather/UXF-Control-Center/pulls)

Please review existing issues before submitting duplicate hardware reports.

---

## License & Attribution

UXF-Control-Center is licensed under the [GNU General Public License v3.0 or later (GPL-3.0-or-later)](./LICENSE).

- **Project Maintainer:** Urxfather (`UXF⚡`)
- **Original Upstream Base:** Derived from `acer-control-center` by Stanly Silas and contributors under GPL-3.0-or-later. Original copyright notices and licensing attributions are preserved in respective source headers.
- **Third-Party Libraries:** Lucide icons ([LUCIDE-LICENSE](file:///home/uxf01/Downloads/acer-control-center-source/gui/resources/icons/LUCIDE-LICENSE)), Tauri, React, and related open-source crates and packages.
