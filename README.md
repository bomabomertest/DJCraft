# sterato-rs

An independent DJ application written in Rust, in the feature class of professional DJ software.
Not affiliated with or endorsed by Serato; "Serato" is a trademark of its owner.

## Status
Early engine work. Working and tested: deck playback (tempo, cues, beat loops), 3-band EQ,
crossfader curves, BPM estimation, baseline stem separation, a CLI. No audio output or UI yet.
See [FEATURES.md](FEATURES.md) for the full target feature list and progress, and
[SPEC.md](SPEC.md) for the implementation policy.

## Layout
- `crates/dj-core`: decks, EQ, mixer, BPM analysis (no dependencies)
- `crates/dj-stems`: `StemSeparator` trait, spectral baseline separator, `StemDeck`
- `crates/dj-cli`: `dj analyze <file>` and `dj split <file> [outdir]`

## Build
    cargo test --workspace --release
    cargo run --release -p dj-cli -- analyze song.mp3
    cargo run --release -p dj-cli -- split song.mp3 stems/

Licence: crates declare `MIT OR Apache-2.0`; add the licence files before publishing.
