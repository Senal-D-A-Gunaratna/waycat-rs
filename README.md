# waycat-rs

A Rust reimplementation of `catloop.sh` from
[CarloCattano/waycat](https://github.com/CarloCattano/waycat) — a
CPU-driven cat animation for waybar, ported from bash to a single
standalone binary. Only the cat is ported; `skull.sh` was not.

## What it does

Reads `/proc/stat`, computes CPU usage from the delta between samples,
and prints a stream of single-letter animation frames on a timing
loop, matching waybar's plain-text custom-module protocol. The cat has
5 "awake" frames and 8 "sleep" frames; it drops into the sleep cycle
after 4 consecutive low-CPU samples, same threshold and speed curve as
the original bash script (`1 / (4 + usage*100)`, clamped to a 0.03s
floor).

Click-to-sleep: `waycat-rs toggle` sends `SIGUSR1` to the running
daemon, which flips an in-memory flag that forces the sleep animation
regardless of CPU load — no lock file, nothing written to disk or
tmpfs. While forced-sleep is active, the daemon stops polling
`/proc/stat` entirely and just cycles frames on a fixed cadence;
polling resumes the instant it's toggled back off.

## Build

Requires the `x86_64-unknown-linux-musl` target:

```
rustup target add x86_64-unknown-linux-musl
```

The project's `.cargo/config.toml` pins the default build target to
musl, so a plain release build already produces a fully static binary
with no runtime linker dependency:

```
cargo build --release
```

Binary lands at `target/x86_64-unknown-linux-musl/release/waycat-rs`.
Copy it wherever you like, e.g. `~/.config/scripts/waycat-rs`.

## Fonts

Fonts are **not** bundled into the binary — install them manually,
same as upstream `waycat`:

```
cp fonts/Waycat.ttf ~/.local/share/fonts/
fc-cache -f
```

## Usage

```
waycat-rs           # run the daemon (prints frames on a loop, forever)
waycat-rs toggle     # signal a running daemon to force sleep / wake it back up
```

## Waybar config

```json
  "custom/waycat-rs": {
    "exec": "pkill waycat-rs ; ~/.config/scripts/waycat-rs",
    "on-click": "~/.config/scripts/waycat-rs toggle",
    "spacing": 1,
    "format": "{}",
    "tooltip": false,
  },
```

```css
#custom-waycat-rs {
  font-family: "Waycat", monospace;
  font-size: 16px;
  font-weight: bold;
}
```

## Behavior parity notes

- CPU sampling, delta calc, and speed formula match `catloop.sh`
  exactly.
- No `bc` or `awk` subprocess spawned per frame (the original bash
  script forked both on every tick).
- Toggle state lives only in the daemon's own process memory,
  flipped via a `SIGUSR1` handler — not a file, not shared memory,
  not polled from disk.
