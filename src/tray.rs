use ksni::menu::{MenuItem, StandardItem};
use ksni::{Icon, TextDirection, ToolTip, Tray};
use notify_rust::Notification;
use std::process::Command;
use tokio::sync::mpsc;

use crate::config::AppConfig;
use crate::pomodoro::{PomodoroCommand, PomodoroPhase, PomodoroState};

pub struct PomodoroTray {
    command_tx: mpsc::UnboundedSender<PomodoroCommand>,
    phase: PomodoroPhase,
    running: bool,
    remaining_label: String,
    completed_focus_sessions: u32,
}

impl PomodoroTray {
    pub fn new(command_tx: mpsc::UnboundedSender<PomodoroCommand>) -> Self {
        Self {
            command_tx,
            phase: PomodoroPhase::Focus,
            running: false,
            remaining_label: String::from("25:00"),
            completed_focus_sessions: 0,
        }
    }

    pub fn sync_state(&mut self, state: &PomodoroState) {
        self.phase = state.phase;
        self.running = state.running;
        self.remaining_label = state.remaining_label();
        self.completed_focus_sessions = state.completed_focus_sessions;
    }

    fn phase_name(&self) -> &'static str {
        match self.phase {
            PomodoroPhase::Focus => "Focus",
            PomodoroPhase::Break => "Break",
        }
    }
}

pub fn notify(summary: &str, body: &str) {
    let _ = Notification::new().summary(summary).body(body).show();
}

fn open_config_file() {
    match AppConfig::config_path() {
        Ok(config_path) => {
            let status = Command::new("xdg-open").arg(&config_path).spawn();
            if status.is_err() {
                notify(
                    "FocusFlow",
                    &format!("Could not open config: {}", config_path.display()),
                );
            }
        }
        Err(_) => {
            notify("FocusFlow", "Could not resolve config path.");
        }
    }
}

impl Tray for PomodoroTray {
    fn id(&self) -> String {
        env!("CARGO_PKG_NAME").into()
    }

    fn icon_name(&self) -> String {
        if self.running {
            "alarm-symbolic".into()
        } else {
            "appointment-soon-symbolic".into()
        }
    }

    fn title(&self) -> String {
        format!("FocusFlow - {} {}", self.phase_name(), self.remaining_label)
    }

    fn text_direction(&self) -> TextDirection {
        TextDirection::LeftToRight
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: String::from("FocusFlow"),
            description: format!(
                "{} phase, {} remaining, {} completed focus sessions.",
                self.phase_name(),
                self.remaining_label,
                self.completed_focus_sessions,
            ),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: if self.running {
                    String::from("Pause")
                } else {
                    String::from("Start")
                },
                icon_name: if self.running {
                    String::from("media-playback-pause")
                } else {
                    String::from("media-playback-start")
                },
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.command_tx.send(PomodoroCommand::Toggle);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: String::from("Reset"),
                icon_name: String::from("view-refresh"),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.command_tx.send(PomodoroCommand::Reset);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: String::from("Config"),
                icon_name: String::from("preferences-system"),
                activate: Box::new(|_tray: &mut Self| {
                    open_config_file();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: String::from("Quit"),
                icon_name: String::from("application-exit"),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.command_tx.send(PomodoroCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        Vec::new()
    }
}
