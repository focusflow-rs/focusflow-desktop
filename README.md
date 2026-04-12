<div align="center">

<img width="100%" src="https://capsule-render.vercel.app/api?type=waving&color=ffffff&height=200&section=header&text=FocusFlow%20Desktop&fontSize=50&fontColor=000&animation=twinkling&fontAlignY=40&desc=Tray-based%20Pomodoro%20for%20Linux%20Mint%20Cinnamon&descAlignY=60&descSize=18">

<p align="center">
  <i>A lightweight Pomodoro timer that lives in your system tray, built in Rust with a focus on practical daily use.</i>
</p>

---

### Features

<div align="center">

| Feature | Description |
|:---:|:---|
| Tray-first workflow | Runs in the system tray (SNI) with no required main window |
| Core controls | Start/Pause, Reset, Config submenu, and Quit directly from tray menu |
| Timer engine | Focus/Break state machine with automatic phase transitions |
| Auto-next option | Optional auto-start when switching to the next phase |
| Runtime config menu | Increase/decrease work and break intervals from tray without restart |
| Presets | One-click presets: 25/5, 50/10, 90/20 |
| Live status | Tray title and tooltip update with current phase and remaining time |
| Notifications | Desktop notifications on phase completion via `notify-rust` |
| Configurable intervals | `work_minutes` and `break_minutes` loaded from config file |
| Validation | Config values are clamped to safe ranges (work: 1-180, break: 1-60) |
| Persistent settings | Config stored in user config directory using TOML + Serde |
| Session restore | Last timer state is persisted and restored after app restart |
| Productivity stats | Tracks sessions and focused time for today and all-time |
| Stats controls | Reset today stats or all-time stats directly from tray |
| Finish sound | Fallback chain: `canberra-gtk-play` -> `paplay` -> `pw-play` -> `aplay` -> terminal bell |
| Packaging support | Includes `.desktop` entry and local installer script |

</div>

### Getting Started

To run this project locally, you only need Rust installed.

```bash
# Clone the repository
git clone https://github.com/your-user/focusflow-desktop.git

# Navigate to project directory
cd focusflow-desktop

# Build check
cargo check

# Run the app
cargo run
```

When running, the app lives in the tray. Use `Ctrl+C` in the terminal to stop it.

### Technologies

This project is a native Rust desktop utility focused on low overhead and simple reliability.

<div align="center">

<a href="https://www.rust-lang.org/"><img src="https://skillicons.dev/icons?i=rust" alt="Rust"/></a>
<a href="https://www.kernel.org/"><img src="https://skillicons.dev/icons?i=linux" alt="Linux"/></a>
<a href="https://www.gnu.org/software/bash/"><img src="https://skillicons.dev/icons?i=bash" alt="Shell"/></a>
<a href="https://github.com/"><img src="https://skillicons.dev/icons?i=github" alt="GitHub"/></a>

*Core crates: ksni, tokio, notify-rust, directories, serde, toml*

</div>

### Configuration

The app creates and loads `config.toml` automatically.

Typical Linux path:

```text
~/.config/focusflow-desktop/config.toml
```

Supported keys:

- `work_minutes`
- `break_minutes`
- `sound_on_finish`
- `auto_start_next_phase`

Validation ranges:

- `work_minutes`: 1 to 180
- `break_minutes`: 1 to 60

Example:

```toml
work_minutes = 25
break_minutes = 5
sound_on_finish = true
auto_start_next_phase = false
```

### Project Structure

```text
focusflow-desktop/
├── src/
│   ├── main.rs       # App entrypoint and event orchestration
│   ├── pomodoro.rs   # Timer state machine
│   ├── tray.rs       # Tray UI and tray actions
│   ├── config.rs     # Config loading and saving
│   ├── storage.rs    # Runtime state + daily/all-time stats persistence
│   └── sound.rs      # Finish sound playback fallback
├── packaging/
│   └── focusflow-desktop.desktop
├── scripts/
│   └── install-desktop.sh
├── Cargo.toml
└── README.md
```

### Packaging

Build release binary and install desktop entry locally:

```bash
sh scripts/install-desktop.sh
```

By default, the installer runs `cargo build --release` automatically.

If you want to skip rebuild and install the current release binary as-is:

```bash
sh scripts/install-desktop.sh --no-build
```

Install and enable autostart on login:

```bash
sh scripts/install-desktop.sh --autostart
```

Install and ensure autostart is disabled:

```bash
sh scripts/install-desktop.sh --no-autostart
```

### License

This project is licensed under the MIT License.

<img width="100%" src="https://capsule-render.vercel.app/api?type=waving&color=ffffff&height=120&section=footer"/>

</div>