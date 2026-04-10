use std::time::Duration;

use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy)]
pub struct PomodoroConfig {
    pub work_minutes: u64,
    pub break_minutes: u64,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            work_minutes: 25,
            break_minutes: 5,
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
    Quit,
}

#[derive(Debug, Clone)]
pub enum PomodoroEvent {
    StateChanged(PomodoroState),
    PhaseCompleted(PomodoroState),
}

pub struct PomodoroEngine;

impl PomodoroEngine {
    pub fn spawn(
        config: PomodoroConfig,
        command_rx: mpsc::UnboundedReceiver<PomodoroCommand>,
        event_tx: mpsc::UnboundedSender<PomodoroEvent>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            Self::run(config, command_rx, event_tx).await;
        })
    }

    async fn run(
        config: PomodoroConfig,
        mut command_rx: mpsc::UnboundedReceiver<PomodoroCommand>,
        event_tx: mpsc::UnboundedSender<PomodoroEvent>,
    ) {
        let mut state = PomodoroState::new(config);
        let _ = event_tx.send(PomodoroEvent::StateChanged(state.clone()));

        loop {
            if state.running {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        if state.remaining.as_secs() > 1 {
                            state.remaining = state.remaining.saturating_sub(Duration::from_secs(1));
                            let _ = event_tx.send(PomodoroEvent::StateChanged(state.clone()));
                        } else {
                            state = Self::advance_phase(state, config);
                            let _ = event_tx.send(PomodoroEvent::PhaseCompleted(state.clone()));
                        }
                    }
                    command = command_rx.recv() => {
                        if !Self::apply_command(&mut state, command, config, &event_tx) {
                            break;
                        }
                    }
                }
            } else {
                match command_rx.recv().await {
                    Some(command) => {
                        if !Self::apply_command(&mut state, Some(command), config, &event_tx) {
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
        config: PomodoroConfig,
        event_tx: &mpsc::UnboundedSender<PomodoroEvent>,
    ) -> bool {
        match command {
            Some(PomodoroCommand::Toggle) => {
                state.running = !state.running;
            }
            Some(PomodoroCommand::Reset) => {
                *state = PomodoroState::new(config);
            }
            Some(PomodoroCommand::Quit) | None => {
                return false;
            }
        }

        let _ = event_tx.send(PomodoroEvent::StateChanged(state.clone()));
        true
    }

    fn advance_phase(mut state: PomodoroState, config: PomodoroConfig) -> PomodoroState {
        match state.phase {
            PomodoroPhase::Focus => {
                state.phase = PomodoroPhase::Break;
                state.remaining = Duration::from_secs(config.break_minutes * 60);
                state.running = false;
                state.completed_focus_sessions = state.completed_focus_sessions.saturating_add(1);
            }
            PomodoroPhase::Break => {
                state.phase = PomodoroPhase::Focus;
                state.remaining = Duration::from_secs(config.work_minutes * 60);
                state.running = false;
            }
        }

        state
    }
}

