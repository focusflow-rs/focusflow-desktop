use std::time::Duration;

use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy)]
pub struct PomodoroConfig {
    pub work_minutes: u64,
    pub break_minutes: u64,
    pub auto_start_next_phase: bool,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            work_minutes: 25,
            break_minutes: 5,
            auto_start_next_phase: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PomodoroPhase {
    Focus,
    Break,
}

#[derive(Debug, Clone)]
pub struct PomodoroState {
    pub phase: PomodoroPhase,
    pub running: bool,
    pub remaining: Duration,
    pub completed_focus_sessions: u32,
}

impl PomodoroState {
    pub fn new(config: PomodoroConfig) -> Self {
        Self {
            phase: PomodoroPhase::Focus,
            running: false,
            remaining: Duration::from_secs(config.work_minutes * 60),
            completed_focus_sessions: 0,
        }
    }

    pub fn phase_name(&self) -> &'static str {
        match self.phase {
            PomodoroPhase::Focus => "Focus",
            PomodoroPhase::Break => "Break",
        }
    }

    pub fn remaining_label(&self) -> String {
        let total_seconds = self.remaining.as_secs();
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{minutes:02}:{seconds:02}")
    }
}

#[derive(Debug)]
pub enum PomodoroCommand {
    Toggle,
    Reset,
    Skip,
    SetWorkMinutes(u64),
    SetBreakMinutes(u64),
    SetAutoStartNextPhase(bool),
    Quit,
}

#[derive(Debug, Clone)]
pub enum PomodoroEvent {
    StateChanged(PomodoroState),
    PhaseCompleted {
        completed_phase: PomodoroPhase,
        focused_seconds: u64,
        state: PomodoroState,
    },
}

pub struct PomodoroEngine;

impl PomodoroEngine {
    pub fn spawn(
        config: PomodoroConfig,
        initial_state: Option<PomodoroState>,
        command_rx: mpsc::UnboundedReceiver<PomodoroCommand>,
        event_tx: mpsc::UnboundedSender<PomodoroEvent>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            Self::run(config, initial_state, command_rx, event_tx).await;
        })
    }

    async fn run(
        mut config: PomodoroConfig,
        initial_state: Option<PomodoroState>,
        mut command_rx: mpsc::UnboundedReceiver<PomodoroCommand>,
        event_tx: mpsc::UnboundedSender<PomodoroEvent>,
    ) {
        let mut state = initial_state.unwrap_or_else(|| PomodoroState::new(config));
        let _ = event_tx.send(PomodoroEvent::StateChanged(state.clone()));

        loop {
            if state.running {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        if state.remaining.as_secs() > 1 {
                            state.remaining = state.remaining.saturating_sub(Duration::from_secs(1));
                            let _ = event_tx.send(PomodoroEvent::StateChanged(state.clone()));
                        } else {
                            let (new_state, completed_phase, focused_seconds) = Self::advance_phase(state, config);
                            state = new_state;
                            let _ = event_tx.send(PomodoroEvent::PhaseCompleted {
                                completed_phase,
                                focused_seconds,
                                state: state.clone(),
                            });
                            let _ = event_tx.send(PomodoroEvent::StateChanged(state.clone()));
                        }
                    }
                    command = command_rx.recv() => {
                        if !Self::apply_command(&mut state, command, &mut config, &event_tx) {
                            break;
                        }
                    }
                }
            } else {
                match command_rx.recv().await {
                    Some(command) => {
                        if !Self::apply_command(&mut state, Some(command), &mut config, &event_tx) {
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
    }

    fn apply_command(
        state: &mut PomodoroState,
        command: Option<PomodoroCommand>,
        config: &mut PomodoroConfig,
        event_tx: &mpsc::UnboundedSender<PomodoroEvent>,
    ) -> bool {
        match command {
            Some(PomodoroCommand::Toggle) => {
                state.running = !state.running;
            }
            Some(PomodoroCommand::Reset) => {
                *state = PomodoroState::new(*config);
            }
            Some(PomodoroCommand::Skip) => {
                Self::skip_phase(state, *config);
            }
            Some(PomodoroCommand::SetWorkMinutes(minutes)) => {
                config.work_minutes = minutes;
                if state.phase == PomodoroPhase::Focus {
                    let full_seconds = config.work_minutes.saturating_mul(60).max(1);
                    state.remaining = Duration::from_secs(full_seconds);
                }
            }
            Some(PomodoroCommand::SetBreakMinutes(minutes)) => {
                config.break_minutes = minutes;
                if state.phase == PomodoroPhase::Break {
                    let full_seconds = config.break_minutes.saturating_mul(60).max(1);
                    state.remaining = Duration::from_secs(full_seconds);
                }
            }
            Some(PomodoroCommand::SetAutoStartNextPhase(value)) => {
                config.auto_start_next_phase = value;
            }
            Some(PomodoroCommand::Quit) | None => {
                return false;
            }
        }

        let _ = event_tx.send(PomodoroEvent::StateChanged(state.clone()));
        true
    }

    fn advance_phase(
        mut state: PomodoroState,
        config: PomodoroConfig,
    ) -> (PomodoroState, PomodoroPhase, u64) {
        let completed_phase = state.phase;
        let focused_seconds = if completed_phase == PomodoroPhase::Focus {
            config.work_minutes.saturating_mul(60)
        } else {
            0
        };

        match completed_phase {
            PomodoroPhase::Focus => {
                state.phase = PomodoroPhase::Break;
                state.remaining = Duration::from_secs(config.break_minutes * 60);
                state.running = config.auto_start_next_phase;
                state.completed_focus_sessions = state.completed_focus_sessions.saturating_add(1);
            }
            PomodoroPhase::Break => {
                state.phase = PomodoroPhase::Focus;
                state.remaining = Duration::from_secs(config.work_minutes * 60);
                state.running = config.auto_start_next_phase;
            }
        }

        (state, completed_phase, focused_seconds)
    }

    fn skip_phase(state: &mut PomodoroState, config: PomodoroConfig) {
        match state.phase {
            PomodoroPhase::Focus => {
                state.phase = PomodoroPhase::Break;
                state.remaining =
                    Duration::from_secs(config.break_minutes.saturating_mul(60).max(1));
            }
            PomodoroPhase::Break => {
                state.phase = PomodoroPhase::Focus;
                state.remaining =
                    Duration::from_secs(config.work_minutes.saturating_mul(60).max(1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration as TokioDuration};

    fn event_channel() -> mpsc::UnboundedSender<PomodoroEvent> {
        let (tx, _rx) = mpsc::unbounded_channel();
        tx
    }

    #[test]
    fn default_config_values_are_expected() {
        let cfg = PomodoroConfig::default();
        assert_eq!(cfg.work_minutes, 25);
        assert_eq!(cfg.break_minutes, 5);
        assert!(!cfg.auto_start_next_phase);
    }

    #[test]
    fn new_state_starts_in_focus_with_configured_duration() {
        let cfg = PomodoroConfig {
            work_minutes: 30,
            break_minutes: 10,
            auto_start_next_phase: true,
        };
        let state = PomodoroState::new(cfg);

        assert_eq!(state.phase, PomodoroPhase::Focus);
        assert!(!state.running);
        assert_eq!(state.remaining, Duration::from_secs(30 * 60));
        assert_eq!(state.completed_focus_sessions, 0);
    }

    #[test]
    fn phase_name_and_remaining_label_are_formatted() {
        let mut state = PomodoroState::new(PomodoroConfig::default());
        state.remaining = Duration::from_secs(9 * 60 + 7);
        assert_eq!(state.phase_name(), "Focus");
        assert_eq!(state.remaining_label(), "09:07");

        state.phase = PomodoroPhase::Break;
        assert_eq!(state.phase_name(), "Break");
    }

    #[test]
    fn toggle_command_flips_running_state() {
        let mut state = PomodoroState::new(PomodoroConfig::default());
        let mut cfg = PomodoroConfig::default();
        let tx = event_channel();

        assert!(PomodoroEngine::apply_command(
            &mut state,
            Some(PomodoroCommand::Toggle),
            &mut cfg,
            &tx
        ));
        assert!(state.running);

        assert!(PomodoroEngine::apply_command(
            &mut state,
            Some(PomodoroCommand::Toggle),
            &mut cfg,
            &tx
        ));
        assert!(!state.running);
    }

    #[test]
    fn reset_command_rebuilds_state_from_config() {
        let mut state = PomodoroState {
            phase: PomodoroPhase::Break,
            running: true,
            remaining: Duration::from_secs(13),
            completed_focus_sessions: 4,
        };
        let mut cfg = PomodoroConfig {
            work_minutes: 40,
            break_minutes: 10,
            auto_start_next_phase: false,
        };
        let tx = event_channel();

        assert!(PomodoroEngine::apply_command(
            &mut state,
            Some(PomodoroCommand::Reset),
            &mut cfg,
            &tx
        ));

        assert_eq!(state.phase, PomodoroPhase::Focus);
        assert!(!state.running);
        assert_eq!(state.remaining, Duration::from_secs(40 * 60));
        assert_eq!(state.completed_focus_sessions, 0);
    }

    #[test]
    fn set_work_minutes_updates_focus_remaining_only_in_focus() {
        let mut state = PomodoroState::new(PomodoroConfig::default());
        let mut cfg = PomodoroConfig::default();
        let tx = event_channel();

        assert!(PomodoroEngine::apply_command(
            &mut state,
            Some(PomodoroCommand::SetWorkMinutes(33)),
            &mut cfg,
            &tx
        ));
        assert_eq!(cfg.work_minutes, 33);
        assert_eq!(state.remaining, Duration::from_secs(33 * 60));

        state.phase = PomodoroPhase::Break;
        state.remaining = Duration::from_secs(99);
        assert!(PomodoroEngine::apply_command(
            &mut state,
            Some(PomodoroCommand::SetWorkMinutes(22)),
            &mut cfg,
            &tx
        ));
        assert_eq!(cfg.work_minutes, 22);
        assert_eq!(state.remaining, Duration::from_secs(99));
    }

    #[test]
    fn set_break_minutes_updates_break_remaining_only_in_break() {
        let mut state = PomodoroState::new(PomodoroConfig::default());
        let mut cfg = PomodoroConfig::default();
        let tx = event_channel();

        state.phase = PomodoroPhase::Break;
        assert!(PomodoroEngine::apply_command(
            &mut state,
            Some(PomodoroCommand::SetBreakMinutes(12)),
            &mut cfg,
            &tx
        ));
        assert_eq!(cfg.break_minutes, 12);
        assert_eq!(state.remaining, Duration::from_secs(12 * 60));

        state.phase = PomodoroPhase::Focus;
        state.remaining = Duration::from_secs(77);
        assert!(PomodoroEngine::apply_command(
            &mut state,
            Some(PomodoroCommand::SetBreakMinutes(3)),
            &mut cfg,
            &tx
        ));
        assert_eq!(cfg.break_minutes, 3);
        assert_eq!(state.remaining, Duration::from_secs(77));
    }

    #[test]
    fn set_auto_next_phase_updates_config() {
        let mut state = PomodoroState::new(PomodoroConfig::default());
        let mut cfg = PomodoroConfig::default();
        let tx = event_channel();

        assert!(PomodoroEngine::apply_command(
            &mut state,
            Some(PomodoroCommand::SetAutoStartNextPhase(true)),
            &mut cfg,
            &tx
        ));
        assert!(cfg.auto_start_next_phase);
    }

    #[test]
    fn skip_command_switches_focus_to_break_and_preserves_running_flag() {
        let mut state = PomodoroState::new(PomodoroConfig::default());
        state.running = true;
        let mut cfg = PomodoroConfig {
            work_minutes: 25,
            break_minutes: 7,
            auto_start_next_phase: false,
        };
        let tx = event_channel();

        assert!(PomodoroEngine::apply_command(
            &mut state,
            Some(PomodoroCommand::Skip),
            &mut cfg,
            &tx
        ));
        assert_eq!(state.phase, PomodoroPhase::Break);
        assert_eq!(state.remaining, Duration::from_secs(7 * 60));
        assert!(state.running);
    }

    #[test]
    fn skip_command_switches_break_to_focus_with_minimum_one_second() {
        let mut state = PomodoroState {
            phase: PomodoroPhase::Break,
            running: false,
            remaining: Duration::from_secs(1),
            completed_focus_sessions: 0,
        };
        let mut cfg = PomodoroConfig {
            work_minutes: 0,
            break_minutes: 0,
            auto_start_next_phase: false,
        };
        let tx = event_channel();

        assert!(PomodoroEngine::apply_command(
            &mut state,
            Some(PomodoroCommand::Skip),
            &mut cfg,
            &tx
        ));
        assert_eq!(state.phase, PomodoroPhase::Focus);
        assert_eq!(state.remaining, Duration::from_secs(1));
    }

    #[test]
    fn quit_and_none_commands_stop_engine_loop() {
        let mut state = PomodoroState::new(PomodoroConfig::default());
        let mut cfg = PomodoroConfig::default();
        let tx = event_channel();

        assert!(!PomodoroEngine::apply_command(
            &mut state,
            Some(PomodoroCommand::Quit),
            &mut cfg,
            &tx
        ));

        assert!(!PomodoroEngine::apply_command(
            &mut state, None, &mut cfg, &tx
        ));
    }

    #[test]
    fn advance_phase_focus_to_break_updates_metrics() {
        let state = PomodoroState {
            phase: PomodoroPhase::Focus,
            running: true,
            remaining: Duration::from_secs(1),
            completed_focus_sessions: 3,
        };
        let cfg = PomodoroConfig {
            work_minutes: 50,
            break_minutes: 10,
            auto_start_next_phase: true,
        };

        let (new_state, completed_phase, focused_seconds) =
            PomodoroEngine::advance_phase(state, cfg);

        assert_eq!(completed_phase, PomodoroPhase::Focus);
        assert_eq!(focused_seconds, 50 * 60);
        assert_eq!(new_state.phase, PomodoroPhase::Break);
        assert_eq!(new_state.remaining, Duration::from_secs(10 * 60));
        assert!(new_state.running);
        assert_eq!(new_state.completed_focus_sessions, 4);
    }

    #[test]
    fn advance_phase_break_to_focus_sets_focused_seconds_to_zero() {
        let state = PomodoroState {
            phase: PomodoroPhase::Break,
            running: true,
            remaining: Duration::from_secs(1),
            completed_focus_sessions: 8,
        };
        let cfg = PomodoroConfig {
            work_minutes: 25,
            break_minutes: 5,
            auto_start_next_phase: false,
        };

        let (new_state, completed_phase, focused_seconds) =
            PomodoroEngine::advance_phase(state, cfg);

        assert_eq!(completed_phase, PomodoroPhase::Break);
        assert_eq!(focused_seconds, 0);
        assert_eq!(new_state.phase, PomodoroPhase::Focus);
        assert_eq!(new_state.remaining, Duration::from_secs(25 * 60));
        assert!(!new_state.running);
        assert_eq!(new_state.completed_focus_sessions, 8);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn spawn_emits_initial_state_from_restored_state() {
        let cfg = PomodoroConfig::default();
        let restored = PomodoroState {
            phase: PomodoroPhase::Break,
            running: false,
            remaining: Duration::from_secs(42),
            completed_focus_sessions: 5,
        };

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let handle = PomodoroEngine::spawn(cfg, Some(restored.clone()), command_rx, event_tx);

        let first_event = timeout(TokioDuration::from_millis(300), event_rx.recv())
            .await
            .expect("timed out waiting initial state")
            .expect("event channel closed");

        match first_event {
            PomodoroEvent::StateChanged(state) => {
                assert_eq!(state.phase, restored.phase);
                assert_eq!(state.running, restored.running);
                assert_eq!(state.remaining, restored.remaining);
                assert_eq!(
                    state.completed_focus_sessions,
                    restored.completed_focus_sessions
                );
            }
            _ => panic!("expected StateChanged as first event"),
        }

        command_tx
            .send(PomodoroCommand::Quit)
            .expect("failed to send quit");
        handle.await.expect("engine task failed");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_decrements_remaining_while_running() {
        let cfg = PomodoroConfig {
            work_minutes: 25,
            break_minutes: 5,
            auto_start_next_phase: false,
        };
        let initial = PomodoroState {
            phase: PomodoroPhase::Focus,
            running: true,
            remaining: Duration::from_secs(3),
            completed_focus_sessions: 0,
        };

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let handle = tokio::spawn(async move {
            PomodoroEngine::run(cfg, Some(initial), command_rx, event_tx).await;
        });

        let _ = timeout(TokioDuration::from_millis(300), event_rx.recv())
            .await
            .expect("timed out waiting initial state")
            .expect("event channel closed");

        let second_event = timeout(TokioDuration::from_millis(1500), event_rx.recv())
            .await
            .expect("timed out waiting tick event")
            .expect("event channel closed");

        match second_event {
            PomodoroEvent::StateChanged(state) => {
                assert_eq!(state.phase, PomodoroPhase::Focus);
                assert_eq!(state.remaining, Duration::from_secs(2));
                assert!(state.running);
            }
            _ => panic!("expected StateChanged after tick"),
        }

        command_tx
            .send(PomodoroCommand::Quit)
            .expect("failed to send quit");
        handle.await.expect("engine task failed");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_emits_phase_completed_when_timer_reaches_zero() {
        let cfg = PomodoroConfig {
            work_minutes: 2,
            break_minutes: 3,
            auto_start_next_phase: false,
        };
        let initial = PomodoroState {
            phase: PomodoroPhase::Focus,
            running: true,
            remaining: Duration::from_secs(1),
            completed_focus_sessions: 1,
        };

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let handle = tokio::spawn(async move {
            PomodoroEngine::run(cfg, Some(initial), command_rx, event_tx).await;
        });

        let _ = timeout(TokioDuration::from_millis(300), event_rx.recv())
            .await
            .expect("timed out waiting initial state")
            .expect("event channel closed");

        let completion_event = timeout(TokioDuration::from_millis(1500), event_rx.recv())
            .await
            .expect("timed out waiting completion event")
            .expect("event channel closed");

        match completion_event {
            PomodoroEvent::PhaseCompleted {
                completed_phase,
                focused_seconds,
                state,
            } => {
                assert_eq!(completed_phase, PomodoroPhase::Focus);
                assert_eq!(focused_seconds, 120);
                assert_eq!(state.phase, PomodoroPhase::Break);
                assert_eq!(state.remaining, Duration::from_secs(180));
                assert!(!state.running);
                assert_eq!(state.completed_focus_sessions, 2);
            }
            _ => panic!("expected PhaseCompleted event"),
        }

        command_tx
            .send(PomodoroCommand::Quit)
            .expect("failed to send quit");
        handle.await.expect("engine task failed");
    }
}
