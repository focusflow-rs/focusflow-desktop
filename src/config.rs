use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub const MIN_WORK_MINUTES: u64 = 1;
pub const MAX_WORK_MINUTES: u64 = 180;
pub const MIN_BREAK_MINUTES: u64 = 1;
pub const MAX_BREAK_MINUTES: u64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub work_minutes: u64,
    pub break_minutes: u64,
    pub sound_on_finish: bool,
    pub auto_start_next_phase: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            work_minutes: 25,
            break_minutes: 5,
            sound_on_finish: true,
            auto_start_next_phase: false,
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        if !config_path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let raw_config = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config file at {}", config_path.display()))?;

        let mut config: Self = toml::from_str(&raw_config)
            .with_context(|| format!("failed to parse config file at {}", config_path.display()))?;

        config.sanitize();

        Ok(config)
    }

    pub fn config_path() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("com", "GitHub", "focusflow-desktop")
            .context("failed to resolve config directory")?;

        Ok(project_dirs.config_dir().join("config.toml"))
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        if let Some(parent_dir) = config_path.parent() {
            fs::create_dir_all(parent_dir).with_context(|| {
                format!(
                    "failed to create config directory at {}",
                    parent_dir.display()
                )
            })?;
        }

        let rendered = toml::to_string_pretty(self).context("failed to serialize config")?;
        fs::write(&config_path, rendered)
            .with_context(|| format!("failed to write config file at {}", config_path.display()))?;
        Ok(())
    }

    pub fn sanitize(&mut self) {
        self.work_minutes = self.work_minutes.clamp(MIN_WORK_MINUTES, MAX_WORK_MINUTES);
        self.break_minutes = self
            .break_minutes
            .clamp(MIN_BREAK_MINUTES, MAX_BREAK_MINUTES);
    }

    pub fn set_work_minutes(&mut self, value: u64) {
        self.work_minutes = value.clamp(MIN_WORK_MINUTES, MAX_WORK_MINUTES);
    }

    pub fn set_break_minutes(&mut self, value: u64) {
        self.break_minutes = value.clamp(MIN_BREAK_MINUTES, MAX_BREAK_MINUTES);
    }

    pub fn set_auto_start_next_phase(&mut self, value: bool) {
        self.auto_start_next_phase = value;
    }

    pub fn config_dir() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("com", "GitHub", "focusflow-desktop")
            .context("failed to resolve config directory")?;
        Ok(project_dirs.config_dir().to_path_buf())
    }
}
