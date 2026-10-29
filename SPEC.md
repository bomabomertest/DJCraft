# Clean-room development rules

Target: a DJ application in the feature class of a commercial product (reference: "Serato DJ Pro";
this project's working name is DJCraft). This file records the rules that govern how code and
requirements enter this repository. They apply to every contributor and every tool, including AI
assistants.

## Rules
1. **Inputs allowed:** public user manuals, support pages, marketing feature lists, general
   DSP / DJ literature (for example the RBJ audio-EQ cookbook, Krumhansl-Schmuckler key
   profiles, textbook onset/tempo methods), openly licensed libraries, and your own observation
   of the app's *user-visible behaviour*.
2. **Inputs NOT allowed:** decompiled or disassembled code, memory dumps, leaked or
   unlicensed source, or output from reverse-engineering tools (for example REA, Ghidra,
   Hopper, IDA) run on the original binary. Reading these taints the implementation and defeats
   the clean-room claim. No proprietary implementation details may be copied, transliterated or
   "recreated from memory".
3. **Proprietary file formats and interoperability** (crates, library databases, tag blobs):
   isolate the work. One party writes a *format specification* from lawful analysis; the
   implementer works only from that specification and never sees the original code or tool
   output. Check the product's licence terms and local law before doing any such analysis (this
   is not legal advice).
4. **Provenance:** when adding a non-obvious algorithm, note its public source in the code or in
   `PROGRESS.md`. Algorithms in the repository today (RBJ biquads, onset-envelope autocorrelation,
   Krumhansl-Schmuckler key profiles, median-filter harmonic/percussive masking, constant-power
   crossfade) are standard published techniques implemented from general knowledge.
5. **Feature list provenance:** `FEATURES.md` was compiled from the product's public support
   pages, release notes, retailer pages and hardware quick-start guides only.

## Behavioural spec (current)
- Decks: load, play/pause, seek, tempo (and momentary nudge), 8 cue slots, beat loops with
  halve/double, 8 saved loops, loop roll, beat jump, slip, censor, pitch play.
- Mixer: per-channel gain, 3-band EQ with kill, crossfader (smooth / linear / cut curves),
  master gain, peak meters, limiter with warning flag.
- Analysis: BPM, constant-tempo beatgrid with quantize and sync helpers, musical key (Camelot
  notation), waveform overview with frequency colouring.
- FX / sampler: echo, sweep filter, bit crusher; 8-slot sampler (one-shot / hold / loop).
- Library: in-memory crates, sub-crates, smart crates, search, history, ratings, play counts.
- Not yet built: audio output, UI, keylock, MIDI, DVS, recording, streaming, persistence.

## Stem separation
- `StemSeparator` trait -> drums / bass / vocals / other; stems sum back to the input exactly.
- Baseline `SpectralSeparator`: median-filter harmonic/percussive masks, a ~150-250 Hz band for
  bass, stereo-centre weighting for vocals. It is a **non-ML baseline** for plumbing and tests,
  not production quality; real music will bleed between stems.
- Production path: implement `StemSeparator` over an openly licensed neural model (for example
  Demucs exported to ONNX and run with `ort`). Nothing of the sort exists in this repo yet. Check
  the model's licence before shipping its weights.
- `StemDeck`: four synchronized decks with per-stem gain/mute.
