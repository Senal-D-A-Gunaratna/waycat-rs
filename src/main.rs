use std::env;
use std::fs;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

const SLEEP_AFTER: u32 = 4;

// A..E
const AWAKE_FRAMES: [char; 5] = ['A', 'B', 'C', 'D', 'E'];
// G..N
const SLEEP_FRAMES: [char; 8] = ['G', 'H', 'I', 'J', 'K', 'L', 'M', 'N'];

/// The forced-sleep bit itself. Lives only in this process's memory —
/// no file, no tmpfs, nothing on any filesystem. Flipped exclusively from
/// the SIGUSR1 handler below, which is why it has to be an atomic: signal
/// handlers can run at any point, including mid-instruction on another
/// thread, so a plain `bool` write wouldn't be safe here.
static FORCED_SLEEP: AtomicBool = AtomicBool::new(false);

/// Signal handler for SIGUSR1: flip the bit and return immediately.
/// Only async-signal-safe operations are allowed inside a signal handler
/// (no allocation, no I/O) — an atomic store is one of the few things
/// that's guaranteed safe here.
extern "C" fn handle_toggle(_sig: libc::c_int) {
    let current = FORCED_SLEEP.load(Ordering::Relaxed);
    FORCED_SLEEP.store(!current, Ordering::Relaxed);
}

/// Installs the SIGUSR1 handler using raw libc::signal — no crates.io
/// dependency beyond libc itself, since this is the one C-level FFI call
/// std doesn't expose safely.
fn install_signal_handler() {
    unsafe {
        libc::signal(libc::SIGUSR1, handle_toggle as *const () as usize);
    }
}

/// Scans /proc for a running `waycat-rs` daemon (any PID besides our own)
/// and sends it SIGUSR1. This is what `waycat-rs toggle` does instead of
/// touching a lock file — the "message" is the signal itself, and the
/// state it flips lives only in the target process's RAM.
fn send_toggle_signal() -> io::Result<()> {
    let my_pid = std::process::id();
    let mut sent = false;

    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let name = entry.file_name();
        let name = match name.to_str() {
            Some(n) => n,
            None => continue,
        };
        let pid: i32 = match name.parse() {
            Ok(p) => p,
            Err(_) => continue, // not a PID directory
        };
        if pid as u32 == my_pid {
            continue;
        }

        let comm_path = entry.path().join("comm");
        let comm = match fs::read_to_string(&comm_path) {
            Ok(c) => c,
            Err(_) => continue, // process exited, or unreadable — skip
        };

        if comm.trim() == "waycat-rs" {
            unsafe {
                libc::kill(pid, libc::SIGUSR1);
            }
            sent = true;
        }
    }

    if !sent {
        eprintln!("waycat-rs: no running daemon found to toggle");
    }
    Ok(())
}

/// Reads the aggregate "cpu " line from /proc/stat and returns (active, total) jiffies.
fn read_cpu() -> io::Result<(u64, u64)> {
    let contents = fs::read_to_string("/proc/stat")?;
    let line = contents
        .lines()
        .find(|l| l.starts_with("cpu "))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no cpu line in /proc/stat"))?;

    let fields: Vec<u64> = line
        .split_whitespace()
        .skip(1) // skip "cpu"
        .filter_map(|f| f.parse::<u64>().ok())
        .collect();

    // user nice system idle [iowait irq softirq steal guest guest_nice]
    let user = fields.get(0).copied().unwrap_or(0);
    let nice = fields.get(1).copied().unwrap_or(0);
    let sys = fields.get(2).copied().unwrap_or(0);
    let idle = fields.get(3).copied().unwrap_or(0);

    let active = user + nice + sys;
    let total = active + idle;

    Ok((active, total))
}

fn emit(frame: char, speed: f64) {
    print!("{frame}\n");
    let _ = io::stdout().flush();
    sleep(Duration::from_secs_f64(speed));
}

fn main() -> io::Result<()> {
    // `waycat-rs toggle` doesn't run the loop at all — it just signals the
    // already-running daemon and exits. This is what waybar's on-click calls.
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) == Some("toggle") {
        return send_toggle_signal();
    }

    install_signal_handler();

    let (mut prev_active, mut prev_total) = read_cpu()?;
    let mut count: u32 = 0;

    // Fixed cadence used while forced-sleep is active, so we don't touch
    // /proc/stat at all during that time.
    const FORCED_SLEEP_SPEED: f64 = 0.2;

    loop {
        if FORCED_SLEEP.load(Ordering::Relaxed) {
            // CAT IS FORCED ASLEEP — no CPU polling while this holds.
            for &f in SLEEP_FRAMES.iter() {
                emit(f, FORCED_SLEEP_SPEED);
                // Bail out mid-animation as soon as it's unset, so toggling
                // again feels responsive instead of waiting out the cycle.
                if !FORCED_SLEEP.load(Ordering::Relaxed) {
                    break;
                }
            }
            // Resync the CPU baseline for when polling resumes, so the
            // first post-wake reading isn't measured across a huge gap.
            if let Ok((a, t)) = read_cpu() {
                prev_active = a;
                prev_total = t;
            }
            count = 0;
            continue;
        }

        let (active, total) = read_cpu()?;

        let delta_active = active as i64 - prev_active as i64;
        let delta_total = total as i64 - prev_total as i64;

        let cpu_usage = if delta_total <= 0 || delta_active < 0 {
            0.0
        } else {
            delta_active as f64 / delta_total as f64
        };

        let mut speed = 1.0 / (4.0 + (cpu_usage * 100.0));
        if speed < 0.03 {
            speed = 0.03;
        }

        if cpu_usage < 0.02 {
            count += 1;
        } else {
            count = 0;
        }

        if count >= SLEEP_AFTER {
            // CAT IS SLEEPING
            for &f in SLEEP_FRAMES.iter() {
                emit(f, speed);
            }
        } else {
            // CAT IS AWAKE
            for &f in AWAKE_FRAMES.iter() {
                emit(f, speed);
            }
        }

        prev_active = active;
        prev_total = total;
    }
}
