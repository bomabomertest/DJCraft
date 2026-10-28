# Implementation policy

Target: a DJ application in the feature class of "Serato DJ Pro" (name as given: "Sterato").

## Policy
1. Code here is written from public documentation, general DSP/DJ literature (e.g. the RBJ
   audio EQ cookbook) and observed user-visible behaviour.
2. Decompiled or disassembled code from the original product is not pasted or transliterated
   into this repo. If reverse engineering is used (e.g. with REA on a lawfully obtained copy),
   record findings as behavioural notes (algorithms, parameters, formats) and write original code.
3. The original's licence forbids reverse engineering and EU law permits decompilation only for
   interoperability; take legal advice before publishing anything derived from such analysis.

## Feature spec (behavioural)
- Decks: load, play/pause, seek, tempo +-, 8 hot cues, beat loops.
- Mixer: per-channel gain, 3-band EQ with kill, crossfader (smooth/linear/cut curves).
- Analysis: BPM, (next) key, beat grid, waveform overview.
- Library, playlists/crates, sync, FX, recording, MIDI controller mapping, UI: not yet built.

## Stem separation
- `StemSeparator` trait -> drums / bass / vocals / other; stems sum back to the input exactly.
- Baseline `SpectralSeparator`: median-filter harmonic/percussive masks, <150-250 Hz band for bass,
  stereo-centre weighting for vocals. It is a non-ML baseline for plumbing and tests, not
  production quality; real music will bleed between stems.
- Production path: implement `StemSeparator` over an openly licensed neural model (e.g. Demucs
  exported to ONNX, run with `ort`). Check the model's licence before shipping its weights.
- `StemDeck`: four synchronized decks with per-stem gain/mute.
