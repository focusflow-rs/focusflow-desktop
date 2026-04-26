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
    fn default_values_are_expected() {
        prepare_test_environment();
        let cfg = AppConfig::default();
        assert_eq!(cfg.work_minutes, 25);
        assert_eq!(cfg.break_minutes, 5);
        assert!(cfg.sound_on_finish);
        assert!(!cfg.auto_start_next_phase);
    }

    #[test]
    fn sanitize_clamps_invalid_values() {
        prepare_test_environment();
        let mut cfg = AppConfig {
            work_minutes: 999,
            break_minutes: 0,
            sound_on_finish: true,
            auto_start_next_phase: false,
        };

        cfg.sanitize();

        assert_eq!(cfg.work_minutes, MAX_WORK_MINUTES);
        assert_eq!(cfg.break_minutes, MIN_BREAK_MINUTES);
    }

    #[test]
    fn set_work_minutes_clamps_bounds() {
        prepare_test_environment();
        let mut cfg = AppConfig::default();

        cfg.set_work_minutes(0);
        assert_eq!(cfg.work_minutes, MIN_WORK_MINUTES);

        cfg.set_work_minutes(MAX_WORK_MINUTES + 200);
        assert_eq!(cfg.work_minutes, MAX_WORK_MINUTES);
    }

    #[test]
    fn set_break_minutes_clamps_bounds() {
        prepare_test_environment();
        let mut cfg = AppConfig::default();

        cfg.set_break_minutes(0);
        assert_eq!(cfg.break_minutes, MIN_BREAK_MINUTES);

        cfg.set_break_minutes(MAX_BREAK_MINUTES + 200);
        assert_eq!(cfg.break_minutes, MAX_BREAK_MINUTES);
    }

    #[test]
    fn set_auto_next_phase_updates_flag() {
        prepare_test_environment();
        let mut cfg = AppConfig::default();
        assert!(!cfg.auto_start_next_phase);

        cfg.set_auto_start_next_phase(true);
        assert!(cfg.auto_start_next_phase);
    }

    #[test]
    fn config_paths_are_resolvable() {
        prepare_test_environment();
        let file_path = AppConfig::config_path().expect("config path should resolve");
        let dir_path = AppConfig::config_dir().expect("config dir should resolve");

        assert!(file_path.ends_with("config.toml"));
        assert!(file_path.starts_with(&dir_path));
    }

    #[test]
    fn save_and_load_roundtrip() {
        prepare_test_environment();
        let _guard = crate::test_sync::io_lock();

        let cfg = AppConfig {
            work_minutes: 45,
            break_minutes: 15,
            sound_on_finish: false,
            auto_start_next_phase: true,
        };
        cfg.save().expect("save should succeed");

        let loaded = AppConfig::load().expect("load should succeed");
        assert_eq!(loaded.work_minutes, 45);
        assert_eq!(loaded.break_minutes, 15);
        assert!(!loaded.sound_on_finish);
        assert!(loaded.auto_start_next_phase);
    }

    #[test]
    fn load_sanitizes_values_from_file() {
        prepare_test_environment();
        let _guard = crate::test_sync::io_lock();

        let path = AppConfig::config_path().expect("config path should resolve");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("should create config dir");
        }

        std::fs::write(
            &path,
            "work_minutes = 999\nbreak_minutes = 0\nsound_on_finish = true\nauto_start_next_phase = false\n",
        )
        .expect("should write config file");

        let loaded = AppConfig::load().expect("load should succeed");
        assert_eq!(loaded.work_minutes, MAX_WORK_MINUTES);
        assert_eq!(loaded.break_minutes, MIN_BREAK_MINUTES);
    }

    #[test]
    fn load_returns_error_for_invalid_toml() {
        prepare_test_environment();
        let _guard = crate::test_sync::io_lock();

        let path = AppConfig::config_path().expect("config path should resolve");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("should create config dir");
        }

        std::fs::write(&path, "not = [valid toml").expect("should write broken file");
        let result = AppConfig::load();
        assert!(result.is_err());
    }

    #[test]
    fn load_creates_default_when_file_is_missing() {
        prepare_test_environment();
        let _guard = crate::test_sync::io_lock();

        let path = AppConfig::config_path().expect("config path should resolve");
        let _ = std::fs::remove_file(&path);

        let loaded = AppConfig::load().expect("load should succeed");
        assert_eq!(loaded.work_minutes, 25);
        assert_eq!(loaded.break_minutes, 5);
        assert!(path.exists());
    }
}
