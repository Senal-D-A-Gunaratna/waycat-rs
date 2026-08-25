use std::fs;
use std::io::{self, Write};
use std::thread::sleep;
use std::time::Duration;

const SLEEP_AFTER: u32 = 4;

// A..E
const AWAKE_FRAMES: [char; 5] = ['A', 'B', 'C', 'D', 'E'];
// G..N
const SLEEP_FRAMES: [char; 8] = ['G', 'H', 'I', 'J', 'K', 'L', 'M', 'N'];

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
    // Match the bash script's behavior: echo the letter, flush, then sleep.
    print!("{frame}\n");
    let _ = io::stdout().flush();
    sleep(Duration::from_secs_f64(speed));
}

fn main() -> io::Result<()> {
    let (mut prev_active, mut prev_total) = read_cpu()?;
    let mut count: u32 = 0;

    loop {
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
