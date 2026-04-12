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

fn try_play() -> bool {
    let candidates = [
        "/usr/share/sounds/freedesktop/stereo/complete.oga",
        "/usr/share/sounds/freedesktop/stereo/bell.oga",
        "/usr/share/sounds/freedesktop/stereo/message.oga",
        "/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga",
    ];

    for file in candidates {
        if run_silent("paplay", &[file]) {
            return true;
        }

        if run_silent("pw-play", &[file]) {
            return true;
        }

        if run_silent("aplay", &[file]) {
            return true;
        }
    }

    // Keep canberra as a last fallback because it can report success while muted by theme settings.
    if run_silent("canberra-gtk-play", &["-i", "complete"]) {
        return true;
    }

    print!("\x07");
    false
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
