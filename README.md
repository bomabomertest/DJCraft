# DJCraft

An independent DJ application engine written in Rust, aimed at the feature class of professional
DJ software (decks, mixer, analysis, stems, library). It is **early-stage engine work**: there is
no audio output, no GUI and no persistence yet. Everything below describes what exists today,
what has been verified, and what is still to do.

> Not affiliated with or endorsed by Serato; "Serato" is a trademark of its owner. The feature
> list is compiled from public documentation only. See [SPEC.md](SPEC.md) for the clean-room rules
> this project follows.

## Status at a glance

| Area | State |
|---|---|
| Deck engine (play, tempo, cues, loops, slip/roll/censor, pitch play) | Implemented, unit-tested |
| Mixer (3-band EQ, crossfader curves, limiter, meters) | Implemented, unit-tested |
| FX (echo, sweep filter, bit crusher), sampler | Implemented, unit-tested (3 effects only) |
| BPM, beatgrid, key, waveform overview | Implemented, tested on **synthetic** audio only |
| Stem separation | **Baseline non-ML separator only** (see below) |
| Library (crates, smart crates, search, history) | In-memory only, unit-tested |
| Audio device output, GUI, MIDI, DVS, streaming, keylock, recording | Not started |

Full per-feature status: [FEATURES.md](FEATURES.md). Development log: [PROGRESS.md](PROGRESS.md).

## Setup

Requires a Rust toolchain with `cargo`. The workspace was built and tested with **rustc/cargo
1.75.0** on Linux (x86_64); newer stable releases should work but have not been tested here.
`Cargo.lock` is committed so dependency versions are reproducible.

```sh
# install Rust if needed (https://rustup.rs), then:
git clone https://github.com/bomabomertest/DJCraft
cd DJCraft
cargo build --workspace --release
cargo test  --workspace --release
```

No system libraries are needed today. Live audio output (a later step) will need your platform's
audio development files (for example ALSA headers on Linux).

## CLI usage

The binary is `dj` (crate `dj-cli`):

```sh
# Tempo, first-beat position and musical key of an audio file
cargo run --release -p dj-cli -- analyze song.mp3
#   song.mp3: 8.0s @ 22050 Hz, ~120.0 BPM, first beat at 0.001s, key A minor (8A)

# Split into drums.wav, bass.wav, vocals.wav, other.wav (16-bit stereo WAV) in ./stems
cargo run --release -p dj-cli -- split song.mp3 stems/
```

- Decoding uses [Symphonia](https://github.com/pdeljanov/Symphonia): WAV, FLAC, MP3, AAC/M4A and
  OGG Vorbis were exercised (see PROGRESS.md for what was tested and how).
- Errors (missing file, undecodable input) print `error: ...` and exit with status 1.
- Mono files are handled by duplicating the channel.
- `split` writes WAV files to the output directory (default `stems`), creating it if needed.

## Architecture

Cargo workspace, five crates, data flowing from decoded audio to analysis and playback primitives:

```
dj-cli  ──decode (Symphonia)──▶ Track ──▶ dj-core::beatgrid / bpm
   │                              │──▶ dj-analysis (key, waveform overview)
   │                              └──▶ dj-stems  (StemSeparator ──▶ Stems ──▶ StemDeck)
dj-library   (metadata, crates, search; independent of audio)
```

| Crate | Contents |
|---|---|
| `dj-core` | No external dependencies. `deck` (playback, tempo/nudge, 8 cues, loops, saved loops, loop roll, beat jump, slip, censor, pitch play), `mixer` (trim/EQ, crossfader curves, master, peak limiter, peak meters), `eq` (RBJ biquad 3-band), `fx` (echo, sweep filter, crusher), `sampler` (8 slots), `bpm` (onset envelope + autocorrelation), `beatgrid` (grid, quantize, sync helpers, transient-tracking refinement). |
| `dj-analysis` | Key detection (chroma + Krumhansl-Schmuckler profiles, Camelot codes) and waveform overview with 3-band colour energy. Uses `rustfft`. |
| `dj-stems` | `StemSeparator` trait, the baseline `SpectralSeparator`, and `StemDeck` (four locked decks with per-stem gain). Uses `rustfft`. |
| `dj-library` | In-memory library: track metadata, search, crates / sub-crates / smart crates, history, prepare list, ratings, play counts, relocate-missing. No dependencies. |
| `dj-cli` | `analyze` and `split` commands. |

The engine is device-independent: `Deck::render` and `Mixer::mix` fill caller-provided buffers, so
a future audio callback (for example via `cpal`) can drive them.

## Stem separation: what this is and is not

`dj-stems` ships a **baseline, non-neural separator** (`SpectralSeparator`). It works in the
frequency domain: median-filter harmonic/percussive masks, a low-frequency band assigned to bass,
and a stereo-centre weighting for vocals. "Other" is the exact residual, so the four stems always
sum back to the input.

- It exists to provide the plumbing (trait, stem decks, CLI, tests), not production quality.
- It was only verified on **synthetic signals** (sine bass, a mid-range tone, noise bursts).
  On real music, expect considerable bleed between stems. No listening comparison on real
  recordings has been done.
- It is **not** a neural-network separator. A model such as Demucs (run through ONNX Runtime or
  similar) would give far better separation, but **nothing like that is implemented or bundled**.
  The `StemSeparator` trait is the intended integration point; check any model's licence before
  shipping its weights.
- Do not expect results comparable to commercial stem separation.

## Current limitations

- No audio device output, GUI, MIDI mapping, hardware control, recording or streaming.
- No keylock: tempo changes also change pitch.
- BPM/beatgrid assume a single constant tempo between 80 and 160 BPM, and the grid is anchored on
  the strongest transient (a beat, not necessarily the downbeat). Verified on synthetic click
  tracks (including a fractional tempo) and on one synthetic file re-encoded to six formats;
  **not validated on real music**.
- Key detection was verified on synthetic chord progressions only; real music (and relative
  major/minor ambiguity) is untested.
- First-beat times from MP3 and M4A encodes of the test file came out about 0.05 s later than from
  the lossless versions; this is probably codec delay that is not compensated, but that cause has
  not been confirmed.
- Decks are independent objects: there is no sample-locked multi-deck clock, and sync helpers
  (`tempo_sync_ratio`, `beat_sync_position`) are not wired into `Deck`.
- `StemDeck::render` allocates per call, so it is not yet real-time safe.
- Baseline stem separation took 1.7 s for 56 s of 22.05 kHz audio in a release build on the
  development sandbox; longer or higher-rate tracks take proportionally longer. Run it as an
  import-time step, not live.
- Mixer is two-channel; EQ crossover frequencies are fixed; "kill" is -40 dB, not silence.
- Library is in memory only (nothing is saved to disk).

## Planned

Audio output (`cpal`), real-time safe two/four-deck engine with sample-locked sync, keylock/
time-stretch, neural stem separator behind `StemSeparator`, more FX, library persistence and tag
import, waveform rendering and a GUI, MIDI mapping and controller profiles, set recording, and
timecode-vinyl support. See [FEATURES.md](FEATURES.md) for the complete list.

## Licence

Crates declare `MIT OR Apache-2.0` in their manifests, but **no licence files have been added
yet**; add them before publishing or accepting contributions.
