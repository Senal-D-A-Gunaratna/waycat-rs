use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Cat,
    Skull,
}

// Fonts are baked into the binary at compile time, no separate .ttf files
// need to be shipped or manually installed by the user.
static WAYCAT_TTF: &[u8] = include_bytes!("../assets/Waycat.ttf");
static SKULLTYPE_TTF: &[u8] = include_bytes!("../assets/Skulltype.ttf");

/// waycatrs keeps its font copy in its own subfolder under
/// ~/.local/share/fonts, e.g. ~/.local/share/fonts/waycatrs/Waycat.ttf,
/// rather than dumping loose files into the flat font directory.
/// fontconfig scans ~/.local/share/fonts *recursively* by default, so this
/// subfolder is auto-discovered with zero config file edits — nothing
/// outside its own directory is ever touched.
fn waycatrs_font_dir(home: &str) -> PathBuf {
    [home, ".local", "share", "fonts", "waycatrs"].iter().collect()
}

/// Write the given font's bytes into ~/.local/share/fonts/waycatrs if not
/// already present with the same size, and refresh the font cache only
/// when something actually changed. Both fonts can coexist here — cat and
/// skull modules typically run concurrently in waybar, so neither mode
/// touches or removes the other's file.
fn ensure_font_installed(filename: &str, bytes: &[u8]) {
    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => return, // no HOME, nothing sane to do; skip silently
    };

    let font_dir = waycatrs_font_dir(&home);
    let font_path = font_dir.join(filename);

    let needs_write = match fs::metadata(&font_path) {
        Ok(meta) => meta.len() as usize != bytes.len(),
        Err(_) => true,
    };

    if !needs_write {
        return;
    }

    if fs::create_dir_all(&font_dir).is_err() {
        return;
    }
    if fs::write(&font_path, bytes).is_err() {
        return;
    }

    // Best-effort cache refresh; ignore failures (e.g. fc-cache missing).
    let _ = Command::new("fc-cache").arg("-f").output();
}

fn read_cpu() -> io::Result<(u64, u64)> {
    let stat = fs::read_to_string("/proc/stat")?;
    let line = stat
        .lines()
        .find(|l| l.starts_with("cpu "))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no cpu line in /proc/stat"))?;

    let mut fields = line.split_whitespace().skip(1);
    let user: u64 = fields.next().unwrap_or("0").parse().unwrap_or(0);
    let nice: u64 = fields.next().unwrap_or("0").parse().unwrap_or(0);
    let sys: u64 = fields.next().unwrap_or("0").parse().unwrap_or(0);
    let idle: u64 = fields.next().unwrap_or("0").parse().unwrap_or(0);

    let active = user + nice + sys;
    let total = active + idle;
    Ok((active, total))
}

/// Emit a single waybar custom-module JSON line and flush stdout.
fn emit(text: char, cpu_pct: f64, class: &str) {
    // text/class are always plain ASCII letters here, no escaping needed.
    println!(
        "{{\"text\":\"{}\",\"tooltip\":\"CPU: {:.1}%\",\"class\":\"{}\"}}",
        text, cpu_pct, class
    );
    let _ = io::stdout().flush();
}

fn frame(base: u8, offset: u8) -> char {
    (base + offset) as char
}

fn main() {
    let mode = match env::args().nth(1).as_deref() {
        Some("skull") => Mode::Skull,
        _ => Mode::Cat, // default, matches catloop.sh behavior
    };

    // Install only the font this mode needs, straight out of the binary.
    // Both cat's and skull's fonts can coexist in the cache dir since
    // waybar typically runs both modules at once.
    match mode {
        Mode::Cat => ensure_font_installed("Waycat.ttf", WAYCAT_TTF),
        Mode::Skull => ensure_font_installed("Skulltype.ttf", SKULLTYPE_TTF),
    }

    let sleep_after: u32 = 4;
    let mut low_cpu_count: u32 = 0;

    let (mut prev_active, mut prev_total) = read_cpu().unwrap_or((0, 0));

    loop {
        let (active, total) = match read_cpu() {
            Ok(v) => v,
            Err(_) => {
                thread::sleep(Duration::from_millis(500));
                continue;
            }
        };

        let delta_active = active as i64 - prev_active as i64;
        let delta_total = total as i64 - prev_total as i64;

        let cpu_usage: f64 = if delta_total <= 0 || delta_active < 0 {
            0.0
        } else {
            delta_active as f64 / delta_total as f64
        };

        let min_speed = match mode {
            Mode::Cat => 0.03,
            Mode::Skull => 0.05,
        };
        let speed = (1.0 / (4.0 + cpu_usage * 100.0)).max(min_speed);
        let speed_dur = Duration::from_secs_f64(speed);

        if cpu_usage < 0.02 {
            low_cpu_count += 1;
        } else {
            low_cpu_count = 0;
        }

        let sleeping = low_cpu_count >= sleep_after;

        match mode {
            Mode::Cat => {
                // AWAKE_FRAMES A..E, SLEEP_FRAMES G..N
                if sleeping {
                    for i in 0..8u8 {
                        emit(frame(b'G', i), cpu_usage * 100.0, "sleep");
                        thread::sleep(speed_dur);
                    }
                } else {
                    for i in 0..5u8 {
                        emit(frame(b'A', i), cpu_usage * 100.0, "awake");
                        thread::sleep(speed_dur);
                    }
                }
            }
            Mode::Skull => {
                // AWAKE_FRAMES A..S (19), SLEEP_FRAMES a..t (20)
                if sleeping {
                    for i in 0..20u8 {
                        emit(frame(b'a', i), cpu_usage * 100.0, "sleep");
                        thread::sleep(speed_dur);
                    }
                } else if cpu_usage > 0.6 {
                    // AWAKE_FRAMES[0:9] -> A..I
                    for i in 0..9u8 {
                        emit(frame(b'A', i), cpu_usage * 100.0, "busy");
                        thread::sleep(speed_dur);
                    }
                } else {
                    // AWAKE_FRAMES[9:] -> J..S
                    for i in 9..19u8 {
                        emit(frame(b'A', i), cpu_usage * 100.0, "awake");
                        thread::sleep(speed_dur);
                    }
                }
            }
        }

        prev_active = active;
        prev_total = total;
    }
}
