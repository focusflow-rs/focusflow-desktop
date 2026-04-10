mod pomodoro;
mod config;
mod sound;
mod tray;

use anyhow::Result;
use pomodoro::{PomodoroCommand, PomodoroConfig, PomodoroEngine, PomodoroEvent};
use tokio::sync::mpsc;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let app_config = config::AppConfig::load()?;
    let (command_tx, command_rx) = mpsc::unbounded_channel::<PomodoroCommand>();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PomodoroEvent>();

    let tray = tray::PomodoroTray::new(command_tx.clone());
    let tray_handle = ksni::TrayMethods::spawn(tray).await?;

    let pomodoro_config = PomodoroConfig {
        work_minutes: app_config.work_minutes,
        break_minutes: app_config.break_minutes,
    };

    let engine = PomodoroEngine::spawn(pomodoro_config, command_rx, event_tx);

    while let Some(event) = event_rx.recv().await {
        match event {
            PomodoroEvent::StateChanged(state) => {
                tray_handle.update(|tray| tray.sync_state(&state)).await;
            }
            PomodoroEvent::PhaseCompleted(state) => {
                tray::notify(
                    "FocusFlow",
                    &format!("{} phase completed.", state.phase_name()),
                );
                if app_config.sound_on_finish {
                    sound::play_finish_sound();
                }
                tray_handle.update(|tray| tray.sync_state(&state)).await;
            }
        }
    }

    engine.abort();
    Ok(())
}
