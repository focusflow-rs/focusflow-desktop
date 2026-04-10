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
| Core controls | Start/Pause, Reset, Config, and Quit directly from tray menu |
| Timer engine | Focus/Break state machine with automatic phase transitions |
| Live status | Tray title and tooltip update with current phase and remaining time |
| Notifications | Desktop notifications on phase completion via `notify-rust` |
| Configurable intervals | `work_minutes` and `break_minutes` loaded from config file |
| Persistent settings | Config stored in user config directory using TOML + Serde |
| Finish sound | Fallback chain: `canberra-gtk-play` -> `paplay` -> terminal bell |

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

Example:

```toml
work_minutes = 25
break_minutes = 5
sound_on_finish = true
```

### Project Structure

```text
focusflow-desktop/
├── src/
│   ├── main.rs       # App entrypoint and event orchestration
│   ├── pomodoro.rs   # Timer state machine
│   ├── tray.rs       # Tray UI and tray actions
│   ├── config.rs     # Config loading and saving
│   └── sound.rs      # Finish sound playback fallback
├── Cargo.toml
└── README.md
```

### License

This project is licensed under the MIT License.

<img width="100%" src="https://capsule-render.vercel.app/api?type=waving&color=ffffff&height=120&section=footer"/>

</div>