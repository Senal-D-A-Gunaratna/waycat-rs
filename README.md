# waycatrs

Rust rewrite of `catloop.sh` / `skull.sh` from CarloCattano/waycat, as a single
standalone binary. Both fonts (`Waycat.ttf`, `Skulltype.ttf`) are compiled
directly into the binary via `include_bytes!` — nothing to manually download
or copy.

## Build

```
cd waycatrs
cargo build --release
```

Binary lands at `target/release/waycatrs`. Copy it wherever you like, e.g.
`~/.local/bin/waycatrs`. Fully standalone from there — `assets/` is only
needed at compile time.

## Font install location — fully self-contained

Fonts are written to their own subfolder, `~/.local/share/fonts/waycatrs/`,
on first run of each mode. fontconfig scans `~/.local/share/fonts`
*recursively* by default, so this subfolder is auto-discovered with **zero
config file edits** — the binary never touches `fonts.conf`, `~/.config`,
or anything outside its own subfolder. Nothing is mixed in with your other
installed fonts either, since it's contained in `waycatrs/` rather than
sitting flat in the fonts directory.

Both `Waycat.ttf` and `Skulltype.ttf` are kept side by side once either mode
has run — this is deliberate, since a typical waybar config runs
`custom/cpucat` and `custom/skull` concurrently, and having one mode delete
the other's font out from under it would break whichever module runs second.

Every run after the first is a no-op on the install step — it checks the
file's already there at the right size — so there's no per-launch overhead
in steady state, and `fc-cache -f` only runs the first time a font is
actually written.

## Usage

```
waycatrs cat     # replaces catloop.sh (default if no arg given)
waycatrs skull   # replaces skull.sh
```

Each loop iteration prints one JSON line to stdout and flushes, matching
waybar's streaming custom-module protocol:

```json
{"text":"A","tooltip":"CPU: 12.3%","class":"awake"}
```

`class` is one of `awake`, `sleep`, or (skull-only) `busy` when CPU > 60%,
so you can target `#custom-cpucat.sleep`, `#custom-skull.busy`, etc. in your
waybar CSS if you want state-based styling beyond the base font/color rules.

## Waybar config

Same shape as upstream, just point exec at the compiled binary and add
`"return-type": "json"`:

```json
"custom/cpucat": {
    "exec": "~/.local/bin/waycatrs cat",
    "return-type": "json",
    "spacing": 1
},
"custom/skull": {
    "exec": "~/.local/bin/waycatrs skull",
    "return-type": "json",
    "spacing": 1
}
```

## Behavior parity notes

- CPU sampling, delta calc, and speed formula (`1 / (4 + usage*100)`, min-speed
  clip) match the bash originals exactly.
- Cat: 5 awake frames (A-E), 8 sleep frames (G-N), sleeps after 4 consecutive
  low-CPU samples (<2%).
- Skull: 19 awake frames (A-S) split at CPU>60% into first-9 (A-I, "busy") vs
  remaining (J-S, "awake"), 20 sleep frames (a-t).
- No `bc`, `awk`, subshells, manually-installed fonts, or config file edits —
  one self-contained binary, no shell dependency.


  
# Compiling

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```
