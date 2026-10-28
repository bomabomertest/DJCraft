# Target feature list

Compiled 2026-10-28 from Serato's public support pages (Software Overview for DJ Pro 4.0, release
notes 4.0.0-4.0.4), retailer/feature pages and hardware quick-start guides. Names are the
product's own terms. `[x]` = implemented here, `[~]` = partial, `[ ]` = not started.

## Decks and playback
- [~] 2 virtual decks (4-deck view in the original): play, pause, seek, tempo (`dj-core::deck`)
- [~] Tempo slider with selectable ranges (original: +/-8, 16, 50%)
- [ ] Keylock (tempo change without pitch change), higher-quality "Pitch 'n Time" stretch / key shift
- [ ] Pitch bend / nudge, slip mode (with slip release time), censor (reverse while held)
- [ ] Per-track gain adjust, per-deck level meters, master level, limiter warning
- [ ] Repeat, autoplay next track from crate, eject, previous/next track
- [ ] Original-BPM readout, elapsed/remaining time, platter position indicator

## Cues, loops and pad performance
- [~] 8 cue points per track (set/jump done; names, colours, delete, persistence not)
- [ ] Temporary cue
- [~] Beat loops (done); [ ] auto loop, manual loop, halve/double, 8 saved loops per track, loop roll,
      cue loop, beat jump
- [ ] Pad modes: hot cue, cue loop, roll, auto/manual/saved loop, sampler, slicer, transport/stutter,
      pitch play (cue points at semitone offsets), saved flip, 16-pad (two modes per deck)

## Sync and analysis
- [x] BPM estimation
- [ ] Beatgrid creation and editing, quantize, beat matching display
- [ ] Simple sync (tempo) and smart sync (beatgrid-aware, beat/tempo/armed states)
- [ ] Musical key detection
- [ ] Waveform overview and coloured (frequency-coloured) waveforms, horizontal and vertical views
- [ ] Batch analysis with file-corruption check; analysis settings

## Stems
- [~] Separation into vocals / melody / bass / drums (baseline non-ML separator; `dj-stems`)
- [x] Per-stem gain / mute on a deck (`StemDeck`)
- [ ] Acapella and instrumental one-click toggles, stem levels UI, stem FX / stem pad FX
- [ ] Stems prepare crate (background batch separation, cached stem files), dynamic waveforms
- [ ] Neural separator behind `StemSeparator` (openly licensed model via ONNX)

## Mixer and FX
- [x] Per-channel 3-band EQ with kill, crossfader (smooth / linear / cut curves), master gain
- [ ] 20+ DJ-FX in multiple modes, FX panel, pad FX (echo, braker, repeater...), FX packs
- [ ] Sampler: slots, play modes, repeat, slot volume, sample sync, load from library
- [ ] Flip: record/replay on-the-fly edits, saved flip sequences
- [ ] Set recording; video mixing (separate expansion in the original)

## Library
- [ ] Crates, sub-crates, smart crates, prepare crate, history panel, files panel
- [ ] Search, all-crate search, columns (BPM, key, duration, tags), simple list and album-art grid
- [ ] Play counts, ratings, favourite crates, coloured crates, crate sort, relocate missing files
- [ ] Import from other libraries (e.g. iTunes/Apple Music), metadata scanning
- [ ] Streaming integration (original lists Apple Music and Spotify Premium; one provider at a time)

## Hardware and I/O
- [ ] Audio device output (`cpal`), headphone cue / split cue, master and booth
- [ ] MIDI mapping and MIDI assign mode, controller profiles (original: 90+ devices), plug-and-play
- [ ] Practice / offline mode (no hardware)
- [ ] Vinyl / CDJ timecode control (DVS): internal, relative, absolute playback modes
- [ ] Tempo link with other apps (the original has a "Link" toggle)

## UI
- [ ] Main screen: decks, waveforms, library, FX/sampler/flip/record panels, day/night mode,
      setup screen, CPU and audio-dropout indicators, expansions window

## Not verified
Streaming providers beyond Apple Music and Spotify, exact FX names and counts, and the exact
semantics of "Link" were not confirmed from the sources above.
