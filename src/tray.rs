use std::sync::{Arc, Mutex};

use ksni::menu::{MenuItem, StandardItem, SubMenu};
use ksni::{Icon, TextDirection, ToolTip, Tray};
use notify_rust::Notification;
use std::process::Command;
use std::thread;
use tokio::sync::mpsc;

use crate::config::{
    AppConfig, MAX_BREAK_MINUTES, MAX_WORK_MINUTES, MIN_BREAK_MINUTES, MIN_WORK_MINUTES,
};
use crate::pomodoro::{PomodoroCommand, PomodoroPhase, PomodoroState};
use crate::sound;
use crate::storage::AppStats;

pub struct PomodoroTray {
    command_tx: mpsc::UnboundedSender<PomodoroCommand>,
    shared_config: Arc<Mutex<AppConfig>>,
    shared_stats: Arc<Mutex<AppStats>>,
    phase: PomodoroPhase,
    running: bool,
    remaining_label: String,
    completed_focus_sessions: u32,
}

impl PomodoroTray {
    pub fn new(
        command_tx: mpsc::UnboundedSender<PomodoroCommand>,
        shared_config: Arc<Mutex<AppConfig>>,
        shared_stats: Arc<Mutex<AppStats>>,
    ) -> Self {
        Self {
            command_tx,
            shared_config,
            shared_stats,
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

    fn with_config<R>(&self, f: impl FnOnce(&AppConfig) -> R) -> Option<R> {
        self.shared_config.lock().ok().map(|cfg| f(&cfg))
    }

    fn update_config(&self, updater: impl FnOnce(&mut AppConfig)) {
        if let Ok(mut cfg) = self.shared_config.lock() {
            updater(&mut cfg);
            cfg.sanitize();
            if let Err(error) = cfg.save() {
                notify("FocusFlow", &format!("Could not save config: {error}"));
            }
        }
    }

    fn with_stats<R>(&self, f: impl FnOnce(&AppStats) -> R) -> Option<R> {
        self.shared_stats.lock().ok().map(|stats| f(&stats))
    }

    fn update_stats(&self, updater: impl FnOnce(&mut AppStats)) {
        if let Ok(mut stats) = self.shared_stats.lock() {
            updater(&mut stats);
            if let Err(error) = stats.save() {
                notify("FocusFlow", &format!("Could not save stats: {error}"));
            }
        }
    }

    fn apply_preset(&self, work_minutes: u64, break_minutes: u64) {
        self.update_config(|cfg| {
            cfg.set_work_minutes(work_minutes);
            cfg.set_break_minutes(break_minutes);
        });

        let _ = self
            .command_tx
            .send(PomodoroCommand::SetWorkMinutes(work_minutes));
        let _ = self
            .command_tx
            .send(PomodoroCommand::SetBreakMinutes(break_minutes));
        let _ = self.command_tx.send(PomodoroCommand::Reset);

        notify(
            "FocusFlow",
            &format!(
                "Preset applied: {}m focus / {}m break",
                work_minutes, break_minutes
            ),
        );
    }

    fn reload_config_from_disk(&self) {
        match AppConfig::load() {
            Ok(mut loaded) => {
                loaded.sanitize();
                if let Ok(mut cfg) = self.shared_config.lock() {
                    *cfg = loaded.clone();
                }

                let _ = self
                    .command_tx
                    .send(PomodoroCommand::SetWorkMinutes(loaded.work_minutes));
                let _ = self
                    .command_tx
                    .send(PomodoroCommand::SetBreakMinutes(loaded.break_minutes));

                notify(
                    "FocusFlow",
                    &format!(
                        "Config reloaded: work {}m, break {}m, auto-next {}",
                        loaded.work_minutes,
                        loaded.break_minutes,
                        if loaded.auto_start_next_phase {
                            "on"
                        } else {
                            "off"
                        }
                    ),
                );

                let _ = self.command_tx.send(PomodoroCommand::SetAutoStartNextPhase(
                    loaded.auto_start_next_phase,
                ));
            }
            Err(error) => {
                notify("FocusFlow", &format!("Could not reload config: {error}"));
            }
        }
    }

    fn show_status(&self) {
        let (work, break_) = self
            .with_config(|cfg| (cfg.work_minutes, cfg.break_minutes))
            .unwrap_or((25, 5));

        let (today_sessions, today_seconds, total_sessions, total_seconds) = self
            .with_stats(|stats| {
                (
                    stats.focus_sessions_today,
                    stats.focus_seconds_today,
                    stats.total_focus_sessions,
                    stats.total_focus_seconds,
                )
            })
            .unwrap_or((0, 0, 0, 0));

        notify(
            "FocusFlow Status",
            &format!(
                "{} phase, {} remaining ({})\nToday: {} sessions, {}\nAll-time: {} sessions, {}\nConfig: work {}m, break {}m",
                self.phase_name(),
                self.remaining_label,
                if self.running { "running" } else { "paused" },
                today_sessions,
                format_focus_time(today_seconds),
                total_sessions,
                format_focus_time(total_seconds),
                work,
                break_
            ),
        );
    }
}

pub fn notify(summary: &str, body: &str) {
    let summary = summary.to_string();
    let body = body.to_string();

    thread::spawn(move || {
        let _ = Notification::new().summary(&summary).body(&body).show();
    });
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

fn format_focus_time(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    format!("{hours:02}h {minutes:02}m")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    fn prepare_test_environment() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let base = std::env::temp_dir().join("focusflow-desktop-tests");
            let _ = std::fs::create_dir_all(&base);
            std::env::set_var("XDG_CONFIG_HOME", &base);
        });
    }

    fn make_tray_with_receiver() -> (PomodoroTray, mpsc::UnboundedReceiver<PomodoroCommand>) {
        prepare_test_environment();

        let (tx, rx) = mpsc::unbounded_channel();

        let cfg = AppConfig {
            work_minutes: 30,
            break_minutes: 10,
            sound_on_finish: true,
            auto_start_next_phase: false,
        };
        let stats = AppStats {
            day_index_utc: 0,
            focus_sessions_today: 2,
            focus_seconds_today: 1800,
            total_focus_sessions: 15,
            total_focus_seconds: 20_000,
        };

        (
            PomodoroTray::new(tx, Arc::new(Mutex::new(cfg)), Arc::new(Mutex::new(stats))),
            rx,
        )
    }

    fn find_standard_item<'a>(
        items: &'a [MenuItem<PomodoroTray>],
        label: &str,
    ) -> Option<&'a StandardItem<PomodoroTray>> {
        items.iter().find_map(|item| match item {
            MenuItem::Standard(standard) if standard.label == label => Some(standard),
            _ => None,
        })
    }

    fn activate_menu_item_by_label(tray: &mut PomodoroTray, label: &str) -> bool {
        fn activate_recursive(
            items: &[MenuItem<PomodoroTray>],
            tray: &mut PomodoroTray,
            label: &str,
        ) -> bool {
            for item in items {
                match item {
                    MenuItem::Standard(standard) if standard.label == label => {
                        (standard.activate)(tray);
                        return true;
                    }
                    MenuItem::SubMenu(submenu) => {
                        if activate_recursive(&submenu.submenu, tray, label) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            false
        }

        let menu = tray.menu();
        activate_recursive(&menu, tray, label)
    }

    #[test]
    fn format_focus_time_formats_hours_and_minutes() {
        assert_eq!(format_focus_time(0), "00h 00m");
        assert_eq!(format_focus_time(65 * 60), "01h 05m");
    }

    #[test]
    fn sync_state_updates_phase_running_and_remaining() {
        let (mut tray, _rx) = make_tray_with_receiver();
        let state = PomodoroState {
            phase: PomodoroPhase::Break,
            running: true,
            remaining: std::time::Duration::from_secs(9 * 60 + 5),
            completed_focus_sessions: 4,
        };

        tray.sync_state(&state);

        assert_eq!(tray.phase, PomodoroPhase::Break);
        assert!(tray.running);
        assert_eq!(tray.remaining_label, "09:05");
        assert_eq!(tray.completed_focus_sessions, 4);
    }

    #[test]
    fn tray_visual_methods_reflect_current_state() {
        let (mut tray, _rx) = make_tray_with_receiver();
        let state = PomodoroState {
            phase: PomodoroPhase::Focus,
            running: false,
            remaining: std::time::Duration::from_secs(5 * 60),
            completed_focus_sessions: 1,
        };
        tray.sync_state(&state);

        assert_eq!(tray.id(), "focusflow-desktop");
        assert_eq!(tray.icon_name(), "alarm-symbolic");
        assert_eq!(tray.title(), "FF F 05:00");
        assert_eq!(tray.text_direction(), TextDirection::LeftToRight);
        assert!(tray.icon_pixmap().is_empty());
    }

    #[test]
    fn tooltip_contains_status_and_config_summary() {
        let (mut tray, _rx) = make_tray_with_receiver();
        let state = PomodoroState {
            phase: PomodoroPhase::Focus,
            running: true,
            remaining: std::time::Duration::from_secs(10 * 60),
            completed_focus_sessions: 3,
        };
        tray.sync_state(&state);

        let tooltip = tray.tool_tip();

        assert_eq!(tooltip.title, "FocusFlow");
        assert!(tooltip.description.contains("Focus phase"));
        assert!(tooltip.description.contains("10:00 remaining"));
        assert!(tooltip.description.contains("Sessions this run: 3"));
        assert!(tooltip.description.contains("Config: work 30m, break 10m"));
    }

    #[test]
    fn menu_contains_expected_primary_actions() {
        let (tray, _rx) = make_tray_with_receiver();
        let menu = tray.menu();

        assert!(find_standard_item(&menu, "Start").is_some());
        assert!(find_standard_item(&menu, "Reset").is_some());
        assert!(find_standard_item(&menu, "Skip current phase").is_some());
        assert!(find_standard_item(&menu, "Show status").is_some());
        assert!(find_standard_item(&menu, "Quit").is_some());
    }

    #[test]
    fn menu_shows_pause_when_running() {
        let (mut tray, _rx) = make_tray_with_receiver();
        let state = PomodoroState {
            phase: PomodoroPhase::Focus,
            running: true,
            remaining: std::time::Duration::from_secs(2 * 60),
            completed_focus_sessions: 0,
        };
        tray.sync_state(&state);

        let menu = tray.menu();
        assert!(find_standard_item(&menu, "Pause").is_some());
        assert!(find_standard_item(&menu, "Start").is_none());
    }

    #[test]
    fn start_reset_skip_and_quit_actions_send_expected_commands() {
        let (mut tray, mut rx) = make_tray_with_receiver();
        let menu = tray.menu();

        let start = find_standard_item(&menu, "Start").expect("missing Start item");
        (start.activate)(&mut tray);
        assert!(matches!(rx.try_recv(), Ok(PomodoroCommand::Toggle)));

        let reset = find_standard_item(&menu, "Reset").expect("missing Reset item");
        (reset.activate)(&mut tray);
        assert!(matches!(rx.try_recv(), Ok(PomodoroCommand::Reset)));

        let skip = find_standard_item(&menu, "Skip current phase").expect("missing Skip item");
        (skip.activate)(&mut tray);
        assert!(matches!(rx.try_recv(), Ok(PomodoroCommand::Skip)));

        let quit = find_standard_item(&menu, "Quit").expect("missing Quit item");
        (quit.activate)(&mut tray);
        assert!(matches!(rx.try_recv(), Ok(PomodoroCommand::Quit)));
    }

    #[test]
    fn submenu_actions_apply_expected_changes() {
        let _guard = crate::test_sync::io_lock();
        let (mut tray, mut rx) = make_tray_with_receiver();

        assert!(activate_menu_item_by_label(&mut tray, "Work +5 min"));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetWorkMinutes(35))
        ));

        assert!(activate_menu_item_by_label(&mut tray, "Break +1 min"));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetBreakMinutes(11))
        ));

        assert!(activate_menu_item_by_label(
            &mut tray,
            "Auto-start next phase: off"
        ));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetAutoStartNextPhase(true))
        ));

        assert!(activate_menu_item_by_label(&mut tray, "25 / 5"));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetWorkMinutes(25))
        ));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetBreakMinutes(5))
        ));
        assert!(matches!(rx.try_recv(), Ok(PomodoroCommand::Reset)));
    }

    #[test]
    fn stats_and_status_actions_execute_without_command_side_effects() {
        let _guard = crate::test_sync::io_lock();
        let (mut tray, mut rx) = make_tray_with_receiver();

        assert!(activate_menu_item_by_label(&mut tray, "Show status"));
        assert!(rx.try_recv().is_err());

        assert!(activate_menu_item_by_label(&mut tray, "Reset today stats"));
        assert!(rx.try_recv().is_err());

        assert!(activate_menu_item_by_label(&mut tray, "Reset all stats"));
        assert!(rx.try_recv().is_err());

        let stats = tray.shared_stats.lock().expect("stats lock should work");
        assert_eq!(stats.focus_sessions_today, 0);
        assert_eq!(stats.focus_seconds_today, 0);
        assert_eq!(stats.total_focus_sessions, 0);
        assert_eq!(stats.total_focus_seconds, 0);
    }

    #[test]
    fn icon_name_covers_all_phase_running_combinations() {
        let (mut tray, _rx) = make_tray_with_receiver();

        tray.sync_state(&PomodoroState {
            phase: PomodoroPhase::Focus,
            running: true,
            remaining: std::time::Duration::from_secs(60),
            completed_focus_sessions: 0,
        });
        assert_eq!(tray.icon_name(), "media-record-symbolic");

        tray.sync_state(&PomodoroState {
            phase: PomodoroPhase::Break,
            running: true,
            remaining: std::time::Duration::from_secs(60),
            completed_focus_sessions: 0,
        });
        assert_eq!(tray.icon_name(), "face-smile-symbolic");

        tray.sync_state(&PomodoroState {
            phase: PomodoroPhase::Break,
            running: false,
            remaining: std::time::Duration::from_secs(60),
            completed_focus_sessions: 0,
        });
        assert_eq!(tray.icon_name(), "appointment-soon-symbolic");
    }

    #[test]
    fn additional_menu_actions_emit_expected_commands() {
        let _guard = crate::test_sync::io_lock();
        let (mut tray, mut rx) = make_tray_with_receiver();

        assert!(activate_menu_item_by_label(
            &mut tray,
            "Work: 30 min (click -5)"
        ));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetWorkMinutes(25))
        ));

        assert!(activate_menu_item_by_label(
            &mut tray,
            "Break: 10 min (click -1)"
        ));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetBreakMinutes(9))
        ));

        assert!(activate_menu_item_by_label(&mut tray, "50 / 10"));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetWorkMinutes(50))
        ));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetBreakMinutes(10))
        ));
        assert!(matches!(rx.try_recv(), Ok(PomodoroCommand::Reset)));

        assert!(activate_menu_item_by_label(&mut tray, "90 / 20"));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetWorkMinutes(90))
        ));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetBreakMinutes(20))
        ));
        assert!(matches!(rx.try_recv(), Ok(PomodoroCommand::Reset)));
    }

    #[test]
    fn reload_config_action_reads_file_and_dispatches_updates() {
        let _guard = crate::test_sync::io_lock();
        let (mut tray, mut rx) = make_tray_with_receiver();

        let config_path = AppConfig::config_path().expect("config path should resolve");
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).expect("should create config dir");
        }
        std::fs::write(
            &config_path,
            "work_minutes = 40\nbreak_minutes = 8\nsound_on_finish = true\nauto_start_next_phase = true\n",
        )
        .expect("should write config file");

        assert!(activate_menu_item_by_label(
            &mut tray,
            "Reload config from file"
        ));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetWorkMinutes(40))
        ));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetBreakMinutes(8))
        ));
        assert!(matches!(
            rx.try_recv(),
            Ok(PomodoroCommand::SetAutoStartNextPhase(true))
        ));
    }

    #[test]
    fn menu_and_tooltip_use_fallback_values_when_locks_are_poisoned() {
        prepare_test_environment();

        let (tx, _rx) = mpsc::unbounded_channel();

        let poisoned_config = Arc::new(Mutex::new(AppConfig::default()));
        let cfg_ref = poisoned_config.clone();
        let _ = std::thread::spawn(move || {
            let _guard = cfg_ref.lock().expect("lock should work before poisoning");
            panic!("intentional poison");
        })
        .join();

        let poisoned_stats = Arc::new(Mutex::new(AppStats::default()));
        let stats_ref = poisoned_stats.clone();
        let _ = std::thread::spawn(move || {
            let _guard = stats_ref.lock().expect("lock should work before poisoning");
            panic!("intentional poison");
        })
        .join();

        let tray = PomodoroTray::new(tx, poisoned_config, poisoned_stats);
        let tooltip = tray.tool_tip();
        let menu = tray.menu();

        assert!(tooltip.description.contains("Config: work 25m, break 5m"));
        assert!(find_standard_item(&menu, "Start").is_some());
    }
}

impl Tray for PomodoroTray {
    fn id(&self) -> String {
        env!("CARGO_PKG_NAME").into()
    }

    fn icon_name(&self) -> String {
        match (self.phase, self.running) {
            (PomodoroPhase::Focus, true) => "media-record-symbolic".into(),
            (PomodoroPhase::Focus, false) => "alarm-symbolic".into(),
            (PomodoroPhase::Break, true) => "face-smile-symbolic".into(),
            (PomodoroPhase::Break, false) => "appointment-soon-symbolic".into(),
        }
    }

    fn title(&self) -> String {
        let phase_short = match self.phase {
            PomodoroPhase::Focus => "F",
            PomodoroPhase::Break => "B",
        };
        format!("FF {phase_short} {}", self.remaining_label)
    }

    fn text_direction(&self) -> TextDirection {
        TextDirection::LeftToRight
    }

    fn tool_tip(&self) -> ToolTip {
        let (work, break_, sound_on, auto_start_next_phase) = self
            .with_config(|cfg| {
                (
                    cfg.work_minutes,
                    cfg.break_minutes,
                    cfg.sound_on_finish,
                    cfg.auto_start_next_phase,
                )
            })
            .unwrap_or((25, 5, true, false));
        let (today_sessions, today_seconds) = self
            .with_stats(|stats| (stats.focus_sessions_today, stats.focus_seconds_today))
            .unwrap_or((0, 0));

        ToolTip {
            title: String::from("FocusFlow"),
            description: format!(
                "{} phase, {} remaining.\nSessions this run: {}\nToday: {} sessions, {}\nConfig: work {}m, break {}m, sound {}{}",
                self.phase_name(),
                self.remaining_label,
                self.completed_focus_sessions,
                today_sessions,
                format_focus_time(today_seconds),
                work,
                break_,
                if sound_on { "on" } else { "off" },
                if auto_start_next_phase { ", auto-next on" } else { ", auto-next off" },
            ),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let (work, break_, sound_on, auto_start_next_phase) = self
            .with_config(|cfg| {
                (
                    cfg.work_minutes,
                    cfg.break_minutes,
                    cfg.sound_on_finish,
                    cfg.auto_start_next_phase,
                )
            })
            .unwrap_or((25, 5, true, false));

        let (today_sessions, today_seconds, total_sessions, total_seconds) = self
            .with_stats(|stats| {
                (
                    stats.focus_sessions_today,
                    stats.focus_seconds_today,
                    stats.total_focus_sessions,
                    stats.total_focus_seconds,
                )
            })
            .unwrap_or((0, 0, 0, 0));

        vec![
            StandardItem {
                label: format!(
                    "{} {} ({})",
                    self.phase_name(),
                    self.remaining_label,
                    if self.running { "running" } else { "paused" }
                ),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
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
                label: String::from("Skip current phase"),
                icon_name: String::from("media-skip-forward"),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.command_tx.send(PomodoroCommand::Skip);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: String::from("Show status"),
                icon_name: String::from("dialog-information"),
                activate: Box::new(|tray: &mut Self| {
                    tray.show_status();
                }),
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: String::from("Config"),
                submenu: vec![
                    StandardItem {
                        label: format!("Work: {work} min (click -5)"),
                        activate: Box::new(|tray: &mut Self| {
                            let current = tray.with_config(|cfg| cfg.work_minutes).unwrap_or(25);
                            let next = current
                                .saturating_sub(5)
                                .clamp(MIN_WORK_MINUTES, MAX_WORK_MINUTES);
                            tray.update_config(|cfg| cfg.set_work_minutes(next));
                            let _ = tray.command_tx.send(PomodoroCommand::SetWorkMinutes(next));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: String::from("Work +5 min"),
                        activate: Box::new(|tray: &mut Self| {
                            let current = tray.with_config(|cfg| cfg.work_minutes).unwrap_or(25);
                            let next = current
                                .saturating_add(5)
                                .clamp(MIN_WORK_MINUTES, MAX_WORK_MINUTES);
                            tray.update_config(|cfg| cfg.set_work_minutes(next));
                            let _ = tray.command_tx.send(PomodoroCommand::SetWorkMinutes(next));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: format!("Break: {break_} min (click -1)"),
                        activate: Box::new(|tray: &mut Self| {
                            let current = tray.with_config(|cfg| cfg.break_minutes).unwrap_or(5);
                            let next = current
                                .saturating_sub(1)
                                .clamp(MIN_BREAK_MINUTES, MAX_BREAK_MINUTES);
                            tray.update_config(|cfg| cfg.set_break_minutes(next));
                            let _ = tray.command_tx.send(PomodoroCommand::SetBreakMinutes(next));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: String::from("Break +1 min"),
                        activate: Box::new(|tray: &mut Self| {
                            let current = tray.with_config(|cfg| cfg.break_minutes).unwrap_or(5);
                            let next = current
                                .saturating_add(1)
                                .clamp(MIN_BREAK_MINUTES, MAX_BREAK_MINUTES);
                            tray.update_config(|cfg| cfg.set_break_minutes(next));
                            let _ = tray.command_tx.send(PomodoroCommand::SetBreakMinutes(next));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: format!("Finish sound: {}", if sound_on { "on" } else { "off" }),
                        activate: Box::new(|tray: &mut Self| {
                            tray.update_config(|cfg| cfg.sound_on_finish = !cfg.sound_on_finish);
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: format!(
                            "Auto-start next phase: {}",
                            if auto_start_next_phase { "on" } else { "off" }
                        ),
                        activate: Box::new(|tray: &mut Self| {
                            let next = tray
                                .with_config(|cfg| !cfg.auto_start_next_phase)
                                .unwrap_or(true);
                            tray.update_config(|cfg| cfg.set_auto_start_next_phase(next));
                            let _ = tray
                                .command_tx
                                .send(PomodoroCommand::SetAutoStartNextPhase(next));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    SubMenu {
                        label: String::from("Presets"),
                        submenu: vec![
                            StandardItem {
                                label: String::from("25 / 5"),
                                activate: Box::new(|tray: &mut Self| tray.apply_preset(25, 5)),
                                ..Default::default()
                            }
                            .into(),
                            StandardItem {
                                label: String::from("50 / 10"),
                                activate: Box::new(|tray: &mut Self| tray.apply_preset(50, 10)),
                                ..Default::default()
                            }
                            .into(),
                            StandardItem {
                                label: String::from("90 / 20"),
                                activate: Box::new(|tray: &mut Self| tray.apply_preset(90, 20)),
                                ..Default::default()
                            }
                            .into(),
                        ],
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: String::from("Play test sound"),
                        icon_name: String::from("audio-volume-high"),
                        activate: Box::new(|_tray: &mut Self| {
                            sound::play_test_sound();
                        }),
                        ..Default::default()
                    }
                    .into(),
                    MenuItem::Separator,
                    StandardItem {
                        label: String::from("Open config file"),
                        icon_name: String::from("preferences-system"),
                        activate: Box::new(|_tray: &mut Self| {
                            open_config_file();
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: String::from("Reload config from file"),
                        icon_name: String::from("view-refresh"),
                        activate: Box::new(|tray: &mut Self| {
                            tray.reload_config_from_disk();
                        }),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format!(
                    "Today: {today_sessions} sessions, {}",
                    format_focus_time(today_seconds)
                ),
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format!(
                    "All-time: {total_sessions} sessions, {}",
                    format_focus_time(total_seconds)
                ),
                enabled: false,
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: String::from("Stats"),
                submenu: vec![
                    StandardItem {
                        label: String::from("Reset today stats"),
                        activate: Box::new(|tray: &mut Self| {
                            tray.update_stats(|stats| stats.reset_today());
                            notify("FocusFlow", "Today stats reset.");
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: String::from("Reset all stats"),
                        activate: Box::new(|tray: &mut Self| {
                            tray.update_stats(|stats| stats.reset_all());
                            notify("FocusFlow", "All-time stats reset.");
                        }),
                        ..Default::default()
                    }
                    .into(),
                ],
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
