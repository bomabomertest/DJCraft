# Progress log

Last updated 2026-10-29. This file records what has actually been done and verified. Claims here
are limited to what was run; anything not run is listed under "Not verified".

## Work completed

### 1. Foundation (2026-10-28)
- Cargo workspace with `dj-core`, `dj-stems`, `dj-cli`.
- `dj-core`: deck (playback with cubic interpolation, tempo, 8 cues, beat loops), 3-band EQ
  (RBJ biquads), mixer with crossfader curves, onset-envelope + autocorrelation BPM estimation.
- `dj-stems`: `StemSeparator` trait, baseline spectral separator, `StemDeck`.
- `dj-cli`: `analyze` (BPM) and `split` (stems to WAV) using Symphonia for decoding.

### 2. Feature list (2026-10-28)
- Target feature list compiled from public support pages, release notes and retailer/hardware
  guides of the reference product; now maintained with statuses in [FEATURES.md](FEATURES.md).

### 3. Engine expansion (2026-10-28/29)
- Deck: nudge, pitch play, loop halve/double, 8 saved loops, loop roll, beat jump, slip,
  censor, cue delete, eject.
- Mixer: peak limiter with warning flag, channel/master peak meters.
- New `dj-core` modules: `fx` (echo, sweep filter, bit crusher), `sampler` (8 slots, one-shot /
  hold / loop), `beatgrid` (grid, quantize, tempo-sync ratio, beat-phase sync helper).
- New crate `dj-analysis`: musical key detection (chroma + Krumhansl-Schmuckler profiles, Camelot
  codes) and waveform overview with 3-band colour energy.
- New crate `dj-library`: in-memory library with search, crates / sub-crates / smart crates,
  history, prepare list, ratings, play counts, relocate-missing.
- CLI `analyze` now also reports the first beat and key.

### 4. Bug found and fixed during manual testing
- The first beatgrid implementation drifted when the coarse tempo estimate was slightly off
  (a 120 BPM test file was reported as ~119 BPM and the first beat landed at 0.348 s), and a
  second attempt using a raw-sample threshold produced ~237 BPM on a file with sustained tones.
  Both failures were caught by running the CLI on a test file, not by the unit tests of that
  time. The locator was rewritten: it anchors on the strongest transient in a high-passed energy
  flux signal, follows beats forwards and backwards, and fits a line through the beats found. A
  regression test (`refines_fractional_tempo`, 123.7 BPM, drift check at 35 s) was added.

### 5. Repository clean-up (this commit)
- The remote contained only the docs plus `sterato-rs.tar.gz` (a tarball that included a nested
  `.git` directory). The actual sources are now in the tree and the tarball is removed from the
  working tree. It remains reachable in git history (commit `2d85f5d`, 49 KB).
- `README.md` rewritten (overview, setup, CLI, architecture, limitations, plans).
- `FEATURES.md` completed: every item of the original list is present with a status.
- `SPEC.md` restored to the strict clean-room rules (an interim wording that allowed
  reverse-engineering output as "behavioural notes" was removed).
- One missing test added (`deck::eject_resets_deck`) after the review found `eject` untested.

## Tests run and results

Environment: Linux sandbox, rustc 1.75.0 / cargo 1.75.0, release profile. `Cargo.lock` is
committed (symphonia 0.5.4, rustfft 6.4.1, encoding_rs 0.8.35). `symphonia` and `encoding_rs`
were pinned to these versions with `cargo update --precise` so the workspace builds on this older
toolchain.

| Command | Result |
|---|---|
| `cargo clean && cargo build --workspace --release` | Finished, 0 warnings |
| `cargo test --workspace --release` | **42 passed, 0 failed, 0 ignored** |

Per crate: `dj-core` 31 (`beatgrid` 4, `bpm` 2, `deck` 11, `eq` 2, `fx` 4, `mixer` 5,
`sampler` 3), `dj-analysis` 4, `dj-library` 4, `dj-stems` 3, `dj-cli` 0.

All automated tests use synthetic signals (click tracks, sines, noise bursts, synthesized chord
progressions) or small hand-built data. There are no tests on real music.

Manual CLI checks (one synthetic 8 s test file, 120 BPM clicks plus two sustained tones,
re-encoded with ffmpeg):

| Input | `dj analyze` output |
|---|---|
| WAV, FLAC, OGG, mono FLAC | ~120.0 BPM, first beat 0.001 s, key A minor (8A) |
| MP3, M4A | ~120.0 BPM, first beat 0.052 s / 0.048 s (later than lossless; cause not confirmed) |
| 56 s file (the test file looped) | ~120.0 BPM, first beat 0.002 s; analysis 0.05 s |

Other manual checks: `dj split` on the MP3 wrote four WAV files; on the 56 s file it took
1.7 s (release build, 22.05 kHz). A missing file and a non-audio file each printed `error: ...`
and exited with status 1; running with no arguments prints usage.

## Outstanding tasks

1. Audio device output and a real-time safe render path (no allocation per callback).
2. Sample-locked multi-deck engine; wire `tempo_sync_ratio` / `beat_sync_position` into decks.
3. Keylock / time-stretch.
4. Neural stem separator (for example Demucs via ONNX Runtime) behind `StemSeparator`, plus
   batch preparation and caching of stems.
5. Library persistence, tag/metadata import, crate colours/names persistence.
6. Remaining FX, pad modes, manual loops, cue loops, temporary cue, slicer, flip, recording.
7. Waveform rendering and a GUI; MIDI mapping; timecode-vinyl support; streaming.
8. Add licence files; add CI (`cargo build`, `cargo test`, `cargo fmt --check`, clippy).

## Known limitations

See "Current limitations" in [README.md](README.md). Key points: synthetic-only validation,
constant-tempo assumption, baseline (non-neural) stem separation of limited quality, no keylock,
no audio output or UI, in-memory-only library, and an allocating `StemDeck::render`.

## Not verified

- Behaviour on real music (BPM, beat placement, key, stem separation quality).
- Anything involving audio hardware: nothing has been played through a sound device.
- Windows and macOS builds; Rust versions other than 1.75.0.
- `cargo clippy` and `cargo fmt` were not run (clippy is not installed in the sandbox).
- Streaming providers, FX naming/counts and "Link" semantics of the reference product (see
  FEATURES.md).

## Next development steps

1. Build the `cpal` output and a minimal two-deck mixing demo, so the engine can be heard.
2. Collect a small set of royalty-free real tracks and add an analysis regression suite
   (expected BPM / key / first beat) to replace synthetic-only confidence.
3. Prototype the ONNX-based separator behind `StemSeparator` and compare it with the baseline on
   the same tracks (listening test plus simple SDR-style measurement).
4. Persist the library (SQLite) and import tags.
5. Start the GUI once the audio path is stable.
