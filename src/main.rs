mod config;
mod pomodoro;
mod sound;
mod storage;
mod tray;

use anyhow::Result;
use pomodoro::{PomodoroCommand, PomodoroConfig, PomodoroEngine, PomodoroEvent, PomodoroPhase};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let mut app_config = config::AppConfig::load()?;
    app_config.sanitize();

    let stats = storage::AppStats::load()?;
    let shared_config = Arc::new(Mutex::new(app_config.clone()));
    let shared_stats = Arc::new(Mutex::new(stats));

    let (command_tx, command_rx) = mpsc::unbounded_channel::<PomodoroCommand>();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PomodoroEvent>();

    let tray = tray::PomodoroTray::new(
        command_tx.clone(),
        shared_config.clone(),
        shared_stats.clone(),
    );
    let tray_handle = ksni::TrayMethods::spawn(tray).await?;

    let pomodoro_config = PomodoroConfig {
        work_minutes: app_config.work_minutes,
        break_minutes: app_config.break_minutes,
        auto_start_next_phase: app_config.auto_start_next_phase,
    };

    let restored_state =
        storage::AppRuntimeState::load()?.map(|persisted| persisted.into_state(pomodoro_config));

    let engine = PomodoroEngine::spawn(pomodoro_config, restored_state, command_rx, event_tx);

    while let Some(event) = event_rx.recv().await {
        match event {
            PomodoroEvent::StateChanged(state) => {
                let runtime_state = storage::AppRuntimeState::from_state(&state);
                let _ = runtime_state.save();
                tray_handle.update(|tray| tray.sync_state(&state)).await;
            }
            PomodoroEvent::PhaseCompleted {
                completed_phase,
                focused_seconds,
                state,
            } => {
                let runtime_state = storage::AppRuntimeState::from_state(&state);
                let _ = runtime_state.save();

                tray::notify(
                    "FocusFlow",
                    &format!("{} phase completed.", state.phase_name()),
                );

                if completed_phase == PomodoroPhase::Focus {
                    if let Ok(mut stats_guard) = shared_stats.lock() {
                        stats_guard.register_completed_focus_session(focused_seconds);
                        let _ = stats_guard.save();
                    }
                }

                let sound_enabled = shared_config
                    .lock()
                    .map(|cfg| cfg.sound_on_finish)
                    .unwrap_or(true);
                if sound_enabled {
                    sound::play_finish_sound();
                }
                tray_handle.update(|tray| tray.sync_state(&state)).await;
            }
        }
    }

    engine.abort();
    Ok(())
}
