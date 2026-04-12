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
