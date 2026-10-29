# Feature list and status

Target feature set of the reference commercial DJ application, compiled on 2026-10-28 from its
public support pages (Software Overview, release notes 4.0.0-4.0.4), retailer/feature pages and
hardware quick-start guides (no proprietary code or binaries; see [SPEC.md](SPEC.md)).
Feature names follow the reference product's terminology.

## Status legend

| Status | Meaning |
|---|---|
| **Implemented** | Code exists **and** is covered by passing automated tests in this repo. Tests use synthetic audio only, so "implemented" does not mean "validated on real music". |
| **Partial** | Some of the feature exists (stated in the Notes column); the rest is missing. |
| **Planned** | Not started. |
| **Unverified** | Whether or how the reference product offers this was not confirmed from the sources above, so the target itself is uncertain. |

Evidence column names unit tests (`crate::module::test`). Last full run: 42 tests, 0 failures
(see [PROGRESS.md](PROGRESS.md)).

## Decks and playback

| Feature | Status | Notes / evidence |
|---|---|---|
| Virtual deck: load, play, pause, seek, tempo | Implemented | `dj-core::deck`: `plays_at_unity`, `half_tempo_interpolates` (cubic interpolation) |
| 2 decks, 4-deck view | Partial | `Deck` type exists and the mixer has 2 channels; no deck manager, no 4-deck view, no sample-locked clock between decks |
| Tempo slider with selectable ranges (original: +/-8, 16, 50%) | Partial | Tempo ratio clamped to 0.5-2.0; no selectable ranges |
| Keylock (tempo change without pitch change) | Planned | Tempo currently also changes pitch |
| High-quality time-stretch / key shift (expansion in the original) | Planned | |
| Pitch bend / nudge | Implemented | `deck::nudge_speeds_up` |
| Slip mode | Partial | Shadow-playhead slip implemented (`loop_roll_returns_to_slip_position`); slip release time not implemented |
| Censor (reverse while held, resume in place) | Implemented | `deck::censor_reverses_then_resumes_in_place` |
| Per-track gain adjust | Partial | Per-channel gain exists in `Mixer`; per-track stored gain does not |
| Per-deck level meters | Partial | Channel peak values computed (`mixer::limiter_caps_peaks_and_warns` checks channel A peak); no UI |
| Master level | Partial | `Mixer::master` gain applied; no dedicated test; no master meter UI (peak value is computed) |
| Limiter and clipping warning | Implemented | `mixer::limiter_caps_peaks_and_warns`, `quiet_signal_untouched` |
| Repeat, autoplay next track, previous/next track | Planned | |
| Eject | Implemented | `deck::eject_resets_deck` |
| Original-BPM readout, elapsed/remaining time, platter position indicator | Planned | `Deck::position_frames` exists; no display layer |

## Cues, loops and pad performance

| Feature | Status | Notes / evidence |
|---|---|---|
| 8 cue points per track: set, jump, delete | Implemented | `deck::cues_roundtrip_and_delete` |
| Cue names, colours, persistence | Planned | |
| Temporary cue | Planned | |
| Beat loops (auto loop by beat count) | Implemented | `deck::loop_wraps_and_stops_at_end` |
| Manual loop (set in/out points) | Planned | |
| Loop halve / double | Implemented | `deck::halve_double_and_saved_loops` |
| 8 saved loops per track | Implemented | `deck::halve_double_and_saved_loops` (not persisted to disk) |
| Loop roll | Implemented | `deck::loop_roll_returns_to_slip_position` |
| Cue loop | Planned | |
| Beat jump | Implemented | `deck::beat_jump_moves_by_beats` |
| Pitch play (cue points played at semitone offsets) | Implemented | `deck::pitch_play_octave_doubles_rate` (speed and pitch change together) |
| Pad modes: hot cue, cue loop, roll, auto/manual/saved loop, sampler, slicer, transport/stutter, saved flip, 16-pad modes | Planned | Underlying deck/sampler operations exist for several; there is no pad-mapping layer, and slicer, stutter and flip do not exist |

## Sync and analysis

| Feature | Status | Notes / evidence |
|---|---|---|
| BPM estimation | Implemented | `bpm::detects_common_tempos`, `beatgrid::refines_fractional_tempo`; synthetic click tracks only; constant tempo, 80-160 BPM |
| Beatgrid creation | Implemented | `beatgrid::finds_tempo_and_first_beat`, `refines_fractional_tempo` (no drift over 35 s on synthetic data); anchored on the strongest transient, not the downbeat |
| Beatgrid editing | Partial | `BeatGrid::shifted` / `with_bpm` exist (`quantize_and_phase` covers `shifted`); no editor UI |
| Quantize | Partial | `BeatGrid::quantize` implemented and tested; not applied to deck actions |
| Beat matching display | Planned | |
| Simple sync (tempo) | Partial | `tempo_sync_ratio` implemented and tested (`sync_helpers`); not wired into decks |
| Smart sync (beatgrid-aware; beat/tempo/armed states) | Partial | `beat_sync_position` phase alignment tested (`sync_helpers`); no sync state machine, not wired into decks |
| Musical key detection | Implemented | `dj-analysis::detect_c_major_and_a_minor`, `camelot_codes`, `silence_has_no_key`; synthetic chord progressions only |
| Waveform overview data with 3-band colouring | Partial | `dj-analysis::overview_colours_follow_frequency`; data only, nothing renders it |
| Horizontal / vertical waveform views | Planned | |
| Batch analysis with file-corruption check; analysis settings | Planned | CLI analyses one file at a time; decode errors are reported, but no corruption check |

## Stems

| Feature | Status | Notes / evidence |
|---|---|---|
| Separation into vocals / melody / bass / drums | Partial | Baseline **non-neural** `SpectralSeparator` (drums/bass/vocals/other); `stems_sum_to_input`, `separates_bass_and_voice` on synthetic signals. Real-music quality not evaluated; not comparable to commercial separation |
| Neural separator behind `StemSeparator` (for example Demucs via ONNX) | Planned | Nothing implemented or bundled |
| Per-stem gain / mute on a deck | Implemented | `dj-stems::stem_deck_mute_removes_stem` (`StemDeck`) |
| Acapella / instrumental one-click toggles, stem levels UI | Planned | Possible via `StemDeck::set_gain`; no toggle layer or UI |
| Stem FX / stem pad FX | Planned | |
| Stems prepare crate (background batch separation, cached stems) | Planned | `dj split` handles one file and writes WAVs; no batch or cache |
| Dynamic stem waveforms | Planned | |

## Mixer and FX

| Feature | Status | Notes / evidence |
|---|---|---|
| Per-channel 3-band EQ with kill | Implemented | `eq::flat_is_transparent`, `low_kill_attenuates_bass_not_treble` (kill = -40 dB) |
| Crossfader (smooth / linear / cut curves) | Implemented | `mixer::endpoints`, `constant_power_midpoint`, `cut_keeps_both_open_in_middle` |
| 20+ DJ-FX in multiple modes, FX panel, FX packs | Partial | 3 effects: echo, sweep filter, bit crusher (`fx::*` tests); exact FX names and counts in the reference are **unverified**; no FX panel |
| Pad FX (echo, braker, repeater, ...) | Planned | |
| Sampler: slots, play modes, slot volume | Implemented | `sampler::one_shot_plays_once_then_stops`, `hold_stops_on_release_loop_wraps`, `volumes_and_mixing` (8 slots) |
| Sampler: sample sync to deck tempo, load from library | Planned | |
| Flip (record and replay on-the-fly edits, saved flips) | Planned | |
| Set recording | Planned | The CLI has a WAV writer for stems only |
| Video mixing (separate expansion in the original) | Planned | |

## Library

| Feature | Status | Notes / evidence |
|---|---|---|
| Crates, sub-crates | Partial | In-memory structures and tests (`plain_and_smart_crates`); no persistence |
| Smart crates | Partial | Rule evaluation tested (BPM range, key, rating, plays, text); in-memory only |
| Prepare crate | Partial | In-memory list (`plays_history_prepare`); not connected to stem preparation |
| History panel | Partial | History list and play counts tested; no panel |
| Files panel | Planned | |
| Search; search within a crate | Implemented | `search_matches_all_words_case_insensitively`, `plain_and_smart_crates` (`search_in`) |
| Columns (BPM, key, duration, tags), simple list and album-art grid | Planned | Metadata fields exist; no display layer or album art |
| Play counts, ratings | Implemented | `plays_history_prepare`, `sorting_relocating_missing` (rating clamped to 5) |
| Favourite and coloured crates; track colours | Partial | Fields exist on `Crate` / `TrackMeta`; no test |
| Sort by BPM | Implemented | `sorting_relocating_missing` |
| Relocate missing files | Implemented | `sorting_relocating_missing` (path update and missing-file detection) |
| Library persistence (database) | Planned | |
| Import from other libraries (for example iTunes / Apple Music), tag/metadata scanning | Planned | |
| Streaming integration (original lists Apple Music and Spotify Premium; one provider at a time) | Planned | |
| Streaming providers beyond Apple Music and Spotify | Unverified | Not confirmed from the sources above |

## Hardware and I/O

| Feature | Status | Notes / evidence |
|---|---|---|
| Audio device output (`cpal`) | Planned | Engine renders into caller buffers; no device code, and nothing in this repo has been run on real audio hardware |
| Headphone cue / split cue, master and booth outputs | Planned | |
| MIDI mapping, MIDI assign mode, controller profiles (original: 90+ devices), plug-and-play | Planned | |
| Practice / offline mode (no hardware) | Planned | |
| Vinyl / CDJ timecode control (DVS): internal, relative, absolute modes | Planned | |
| Tempo link with other apps (the original has a "Link" toggle) | Unverified | Exact semantics not confirmed |

## UI

| Feature | Status | Notes / evidence |
|---|---|---|
| Main screen: decks, waveforms, library, FX/sampler/flip/record panels | Planned | No GUI exists |
| Day / night mode, setup screen, CPU and audio-dropout indicators, expansions window | Planned | |

## Other implemented items (not in the reference list)

| Item | Status | Notes / evidence |
|---|---|---|
| CLI `dj analyze` (BPM, first beat, key) and `dj split` | Partial | Exercised manually on WAV, FLAC, MP3, OGG, M4A and mono input (see PROGRESS.md); no automated CLI tests |
| Decoding via Symphonia (MP3, FLAC, WAV, AAC/M4A, OGG Vorbis) | Partial | Manual CLI checks only on one synthetic file re-encoded per format; no automated tests, no real-music files |

## Not verified from the reference sources

Streaming providers beyond Apple Music and Spotify, exact FX names and counts, and the exact
semantics of "Link" were not confirmed.
