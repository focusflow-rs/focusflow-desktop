use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;
use crate::pomodoro::{PomodoroConfig, PomodoroPhase, PomodoroState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRuntimeState {
    pub phase: String,
    pub running: bool,
    pub remaining_seconds: u64,
    pub completed_focus_sessions: u32,
}

impl AppRuntimeState {
    pub fn from_state(state: &PomodoroState) -> Self {
        Self {
            phase: match state.phase {
                PomodoroPhase::Focus => "focus".to_string(),
                PomodoroPhase::Break => "break".to_string(),
            },
            running: state.running,
            remaining_seconds: state.remaining.as_secs(),
            completed_focus_sessions: state.completed_focus_sessions,
        }
    }

    pub fn into_state(self, config: PomodoroConfig) -> PomodoroState {
        let phase = if self.phase == "break" {
            PomodoroPhase::Break
        } else {
            PomodoroPhase::Focus
        };

        let phase_total = match phase {
            PomodoroPhase::Focus => config.work_minutes.saturating_mul(60),
            PomodoroPhase::Break => config.break_minutes.saturating_mul(60),
        };

        let remaining_seconds = self.remaining_seconds.clamp(1, phase_total.max(1));

        PomodoroState {
            phase,
            running: self.running,
            remaining: std::time::Duration::from_secs(remaining_seconds),
            completed_focus_sessions: self.completed_focus_sessions,
        }
    }

    pub fn load() -> Result<Option<Self>> {
        let path = runtime_state_path()?;
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read runtime state at {}", path.display()))?;
        let parsed = toml::from_str::<Self>(&content)
            .with_context(|| format!("failed to parse runtime state at {}", path.display()))?;
        Ok(Some(parsed))
    }

    pub fn save(&self) -> Result<()> {
        let path = runtime_state_path()?;
        let body = toml::to_string_pretty(self).context("failed to serialize runtime state")?;
        fs::write(&path, body)
            .with_context(|| format!("failed to write runtime state at {}", path.display()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStats {
    pub day_index_utc: u64,
    pub focus_sessions_today: u32,
    pub focus_seconds_today: u64,
    pub total_focus_sessions: u64,
    pub total_focus_seconds: u64,
}

impl Default for AppStats {
    fn default() -> Self {
        Self {
            day_index_utc: current_day_index_utc(),
            focus_sessions_today: 0,
            focus_seconds_today: 0,
            total_focus_sessions: 0,
            total_focus_seconds: 0,
        }
    }
}

impl AppStats {
    pub fn load() -> Result<Self> {
        let path = stats_path()?;
        if !path.exists() {
            let stats = Self::default();
            stats.save()?;
            return Ok(stats);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read stats at {}", path.display()))?;
        let mut stats = toml::from_str::<Self>(&content)
            .with_context(|| format!("failed to parse stats at {}", path.display()))?;
        stats.rollover_day_if_needed();
        Ok(stats)
    }

    pub fn save(&self) -> Result<()> {
        let path = stats_path()?;
        let body = toml::to_string_pretty(self).context("failed to serialize stats")?;
        fs::write(&path, body)
            .with_context(|| format!("failed to write stats at {}", path.display()))
    }

    pub fn register_completed_focus_session(&mut self, focused_seconds: u64) {
        self.rollover_day_if_needed();
        self.focus_sessions_today = self.focus_sessions_today.saturating_add(1);
        self.focus_seconds_today = self.focus_seconds_today.saturating_add(focused_seconds);
        self.total_focus_sessions = self.total_focus_sessions.saturating_add(1);
        self.total_focus_seconds = self.total_focus_seconds.saturating_add(focused_seconds);
    }

    pub fn reset_today(&mut self) {
        self.rollover_day_if_needed();
        self.focus_sessions_today = 0;
        self.focus_seconds_today = 0;
    }

    pub fn reset_all(&mut self) {
        *self = Self::default();
    }

    fn rollover_day_if_needed(&mut self) {
        let today = current_day_index_utc();
        if today != self.day_index_utc {
            self.day_index_utc = today;
            self.focus_sessions_today = 0;
            self.focus_seconds_today = 0;
        }
    }
}

fn runtime_state_path() -> Result<PathBuf> {
    Ok(AppConfig::config_dir()?.join("runtime_state.toml"))
}

fn stats_path() -> Result<PathBuf> {
    Ok(AppConfig::config_dir()?.join("stats.toml"))
}

fn current_day_index_utc() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0)
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

    #[test]
    fn runtime_state_from_state_serializes_phase_and_fields() {
        prepare_test_environment();
        let state = PomodoroState {
            phase: PomodoroPhase::Break,
            running: true,
            remaining: std::time::Duration::from_secs(123),
            completed_focus_sessions: 7,
        };

        let runtime = AppRuntimeState::from_state(&state);

        assert_eq!(runtime.phase, "break");
        assert!(runtime.running);
        assert_eq!(runtime.remaining_seconds, 123);
        assert_eq!(runtime.completed_focus_sessions, 7);
    }

    #[test]
    fn runtime_state_into_state_maps_invalid_phase_to_focus() {
        prepare_test_environment();
        let runtime = AppRuntimeState {
            phase: "unknown".to_string(),
            running: false,
            remaining_seconds: 10,
            completed_focus_sessions: 2,
        };
        let cfg = PomodoroConfig {
            work_minutes: 25,
            break_minutes: 5,
            auto_start_next_phase: false,
        };

        let state = runtime.into_state(cfg);

        assert_eq!(state.phase, PomodoroPhase::Focus);
        assert_eq!(state.remaining, std::time::Duration::from_secs(10));
        assert_eq!(state.completed_focus_sessions, 2);
    }

    #[test]
    fn runtime_state_into_state_clamps_remaining_range() {
        prepare_test_environment();
        let cfg = PomodoroConfig {
            work_minutes: 2,
            break_minutes: 1,
            auto_start_next_phase: false,
        };

        let too_low = AppRuntimeState {
            phase: "focus".to_string(),
            running: true,
            remaining_seconds: 0,
            completed_focus_sessions: 0,
        }
        .into_state(cfg);
        assert_eq!(too_low.remaining, std::time::Duration::from_secs(1));

        let too_high = AppRuntimeState {
            phase: "break".to_string(),
            running: true,
            remaining_seconds: 999,
            completed_focus_sessions: 0,
        }
        .into_state(cfg);
        assert_eq!(too_high.remaining, std::time::Duration::from_secs(60));
    }

    #[test]
    fn stats_default_starts_empty_for_current_day() {
        prepare_test_environment();
        let stats = AppStats::default();
        assert_eq!(stats.day_index_utc, current_day_index_utc());
        assert_eq!(stats.focus_sessions_today, 0);
        assert_eq!(stats.focus_seconds_today, 0);
        assert_eq!(stats.total_focus_sessions, 0);
        assert_eq!(stats.total_focus_seconds, 0);
    }

    #[test]
    fn register_completed_focus_session_updates_today_and_total() {
        prepare_test_environment();
        let mut stats = AppStats::default();
        stats.register_completed_focus_session(1500);

        assert_eq!(stats.focus_sessions_today, 1);
        assert_eq!(stats.focus_seconds_today, 1500);
        assert_eq!(stats.total_focus_sessions, 1);
        assert_eq!(stats.total_focus_seconds, 1500);
    }

    #[test]
    fn register_completed_focus_session_rolls_over_when_day_changes() {
        prepare_test_environment();
        let mut stats = AppStats {
            day_index_utc: current_day_index_utc().saturating_sub(1),
            focus_sessions_today: 9,
            focus_seconds_today: 999,
            total_focus_sessions: 50,
            total_focus_seconds: 5000,
        };

        stats.register_completed_focus_session(60);

        assert_eq!(stats.day_index_utc, current_day_index_utc());
        assert_eq!(stats.focus_sessions_today, 1);
        assert_eq!(stats.focus_seconds_today, 60);
        assert_eq!(stats.total_focus_sessions, 51);
        assert_eq!(stats.total_focus_seconds, 5060);
    }

    #[test]
    fn reset_today_only_clears_today_metrics() {
        prepare_test_environment();
        let mut stats = AppStats {
            day_index_utc: current_day_index_utc(),
            focus_sessions_today: 3,
            focus_seconds_today: 600,
            total_focus_sessions: 11,
            total_focus_seconds: 3600,
        };

        stats.reset_today();

        assert_eq!(stats.focus_sessions_today, 0);
        assert_eq!(stats.focus_seconds_today, 0);
        assert_eq!(stats.total_focus_sessions, 11);
        assert_eq!(stats.total_focus_seconds, 3600);
    }

    #[test]
    fn reset_all_returns_to_default_shape() {
        prepare_test_environment();
        let mut stats = AppStats {
            day_index_utc: 0,
            focus_sessions_today: 8,
            focus_seconds_today: 1200,
            total_focus_sessions: 20,
            total_focus_seconds: 7200,
        };

        stats.reset_all();

        assert_eq!(stats.day_index_utc, current_day_index_utc());
        assert_eq!(stats.focus_sessions_today, 0);
        assert_eq!(stats.focus_seconds_today, 0);
        assert_eq!(stats.total_focus_sessions, 0);
        assert_eq!(stats.total_focus_seconds, 0);
    }

    #[test]
    fn helper_paths_resolve_expected_filenames() {
        prepare_test_environment();
        let runtime = runtime_state_path().expect("runtime path should resolve");
        let stats = stats_path().expect("stats path should resolve");

        assert!(runtime.ends_with("runtime_state.toml"));
        assert!(stats.ends_with("stats.toml"));
    }

    #[test]
    fn runtime_state_save_and_load_roundtrip() {
        prepare_test_environment();
        let _guard = crate::test_sync::io_lock();

        let state = AppRuntimeState {
            phase: "break".to_string(),
            running: true,
            remaining_seconds: 17,
            completed_focus_sessions: 9,
        };
        state.save().expect("runtime save should succeed");

        let loaded = AppRuntimeState::load()
            .expect("runtime load should succeed")
            .expect("runtime state should exist");

        assert_eq!(loaded.phase, "break");
        assert!(loaded.running);
        assert_eq!(loaded.remaining_seconds, 17);
        assert_eq!(loaded.completed_focus_sessions, 9);
    }

    #[test]
    fn stats_save_and_load_roundtrip() {
        prepare_test_environment();
        let _guard = crate::test_sync::io_lock();

        let stats = AppStats {
            day_index_utc: current_day_index_utc(),
            focus_sessions_today: 5,
            focus_seconds_today: 900,
            total_focus_sessions: 25,
            total_focus_seconds: 7200,
        };
        stats.save().expect("stats save should succeed");

        let loaded = AppStats::load().expect("stats load should succeed");
        assert_eq!(loaded.focus_sessions_today, 5);
        assert_eq!(loaded.focus_seconds_today, 900);
        assert_eq!(loaded.total_focus_sessions, 25);
        assert_eq!(loaded.total_focus_seconds, 7200);
    }

    #[test]
    fn runtime_state_load_returns_none_when_file_missing() {
        prepare_test_environment();
        let _guard = crate::test_sync::io_lock();

        let path = runtime_state_path().expect("runtime path should resolve");
        let _ = std::fs::remove_file(path);

        let loaded = AppRuntimeState::load().expect("runtime load should not fail");
        assert!(loaded.is_none());
    }

    #[test]
    fn stats_load_creates_default_when_file_missing() {
        prepare_test_environment();
        let _guard = crate::test_sync::io_lock();

        let path = stats_path().expect("stats path should resolve");
        let _ = std::fs::remove_file(&path);

        let loaded = AppStats::load().expect("stats load should succeed");
        assert_eq!(loaded.focus_sessions_today, 0);
        assert_eq!(loaded.focus_seconds_today, 0);
        assert_eq!(loaded.total_focus_sessions, 0);
        assert_eq!(loaded.total_focus_seconds, 0);
        assert!(path.exists());
    }

    #[test]
    fn runtime_state_load_returns_error_for_invalid_toml() {
        prepare_test_environment();
        let _guard = crate::test_sync::io_lock();

        let path = runtime_state_path().expect("runtime path should resolve");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("should create runtime dir");
        }
        std::fs::write(&path, "phase = [invalid").expect("should write invalid runtime file");

        let loaded = AppRuntimeState::load();
        assert!(loaded.is_err());
    }
}
