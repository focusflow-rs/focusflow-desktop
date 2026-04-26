use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

static SOUND_FAILURE_WARNED: AtomicBool = AtomicBool::new(false);

fn run_silent(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn try_play_with_runner(mut runner: impl FnMut(&str, &[&str]) -> bool) -> bool {
    let candidates = [
        "/usr/share/sounds/freedesktop/stereo/complete.oga",
        "/usr/share/sounds/freedesktop/stereo/bell.oga",
        "/usr/share/sounds/freedesktop/stereo/message.oga",
        "/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga",
    ];

    for file in candidates {
        if runner("paplay", &[file]) {
            return true;
        }

        if runner("pw-play", &[file]) {
            return true;
        }

        if runner("aplay", &[file]) {
            return true;
        }
    }

    // Keep canberra as a last fallback because it can report success while muted by theme settings.
    if runner("canberra-gtk-play", &["-i", "complete"]) {
        return true;
    }

    print!("\x07");
    false
}

fn try_play() -> bool {
    try_play_with_runner(run_silent)
}

pub fn play_finish_sound() {
    let played = try_play();
    if !played && !SOUND_FAILURE_WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("FocusFlow: could not play finish sound with paplay/pw-play/aplay/canberra.");
    }
}

pub fn play_test_sound() {
    let played = try_play();
    if !played {
        eprintln!("FocusFlow: test sound failed. Check audio output and available players.");
    }
    if played {
        SOUND_FAILURE_WARNED.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_silent_returns_false_for_missing_program() {
        assert!(!run_silent("definitely-not-a-real-command-focusflow", &[]));
    }

    #[test]
    fn run_silent_handles_known_commands() {
        assert!(run_silent("true", &[]));
        assert!(!run_silent("false", &[]));
    }

    #[test]
    fn play_sound_functions_do_not_panic() {
        play_finish_sound();
        play_test_sound();
    }

    #[test]
    fn try_play_with_runner_short_circuits_on_first_success() {
        let mut calls = 0;
        let played = try_play_with_runner(|program, _args| {
            calls += 1;
            program == "paplay"
        });

        assert!(played);
        assert_eq!(calls, 1);
    }

    #[test]
    fn try_play_with_runner_uses_canberra_fallback() {
        let played = try_play_with_runner(|program, _args| program == "canberra-gtk-play");
        assert!(played);
    }

    #[test]
    fn try_play_with_runner_returns_false_when_all_fail() {
        let played = try_play_with_runner(|_, _| false);
        assert!(!played);
    }
}
