//! `dj analyze <file>`: BPM, beatgrid, key.  `dj split <file> [outdir]`: stem separation to WAVs.
use dj_analysis::detect_key;
use dj_core::{beatgrid, deck::Track};
use dj_stems::{SpectralSeparator, StemSeparator, STEM_NAMES};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
    meta::MetadataOptions, probe::Hint,
};

fn decode(path: &str) -> Result<Track, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("no audio track")?.clone();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let sample_rate = track.codec_params.sample_rate.ok_or("unknown sample rate")?;
    let (mut left, mut right) = (Vec::new(), Vec::new());

    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track.id { continue; }
        let Ok(decoded) = decoder.decode(&packet) else { continue };
        let spec = *decoded.spec();
        let ch = spec.channels.count();
        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        for frame in buf.samples().chunks(ch) {
            left.push(frame[0]);
            right.push(if ch > 1 { frame[1] } else { frame[0] });
        }
    }
    Ok(Track { sample_rate, left, right })
}

fn write_wav(path: &std::path::Path, t: &Track) -> std::io::Result<()> {
    use std::io::Write;
    let n = t.frames();
    let data_len = (n * 4) as u32;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(b"RIFF")?; f.write_all(&(36 + data_len).to_le_bytes())?; f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?; f.write_all(&1u16.to_le_bytes())?; f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&t.sample_rate.to_le_bytes())?; f.write_all(&(t.sample_rate * 4).to_le_bytes())?;
    f.write_all(&4u16.to_le_bytes())?; f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?; f.write_all(&data_len.to_le_bytes())?;
    for i in 0..n {
        for x in [t.left[i], t.right[i]] {
            f.write_all(&((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match (args.get(1).map(String::as_str), args.get(2)) {
        (Some("analyze"), Some(path)) => match decode(path) {
            Ok(t) => {
                let mono = t.mono();
                let secs = t.frames() as f32 / t.sample_rate as f32;
                print!("{path}: {secs:.1}s @ {} Hz", t.sample_rate);
                match beatgrid::analyze(&mono, t.sample_rate) {
                    Some(g) => print!(", ~{:.1} BPM, first beat at {:.3}s", g.bpm, g.first_beat / t.sample_rate as f64),
                    None => print!(", BPM undetermined"),
                }
                match detect_key(&mono, t.sample_rate) {
                    Some(k) => println!(", key {} ({})", k.name(), k.camelot()),
                    None => println!(", key undetermined"),
                }
            }
            Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
        },
        (Some("split"), Some(path)) => {
            let out = std::path::PathBuf::from(args.get(3).map(String::as_str).unwrap_or("stems"));
            let run = || -> Result<(), Box<dyn std::error::Error>> {
                std::fs::create_dir_all(&out)?;
                let stems = SpectralSeparator::default().separate(&decode(path)?)?;
                for (name, t) in STEM_NAMES.iter().zip([&stems.drums, &stems.bass, &stems.vocals, &stems.other]) {
                    let p = out.join(format!("{name}.wav"));
                    write_wav(&p, t)?;
                    println!("wrote {}", p.display());
                }
                Ok(())
            };
            if let Err(e) = run() { eprintln!("error: {e}"); std::process::exit(1); }
        }
        _ => eprintln!("usage: dj analyze <audio-file> | dj split <audio-file> [out-dir]"),
    }
}
