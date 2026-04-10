use std::process::Command;

pub fn play_finish_sound() {
    if Command::new("canberra-gtk-play")
        .args(["-i", "complete"])
        .spawn()
        .is_ok()
    {
        return;
    }

    if Command::new("paplay")
        .arg("/usr/share/sounds/freedesktop/stereo/complete.oga")
        .spawn()
        .is_ok()
    {
        return;
    }

    print!("\x07");
}
