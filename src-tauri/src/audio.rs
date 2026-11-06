//! Import mastera: kopia do katalogu danych (nazwa = hash), MIME dla Gemini,
//! dekodowanie przez symphonia → peaks do waveformu i czas trwania.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use symphonia::core::audio::sample::Sample;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::error::{AppError, AppResult};

pub const PEAK_BINS: usize = 600;
/// Co ile ramek liczymy jeden peak przy dekodowaniu (potem zmniejszane do PEAK_BINS).
const BLOCK: usize = 2048;

#[derive(Debug, Clone, Serialize)]
pub struct ImportedAudio {
    pub path: String,
    pub hash: String,
    pub size: u64,
    pub mime: String,
    pub file_name: String,
}

/// MIME akceptowane przez Gemini (jak `mimeOf` z prototypu).
pub fn mime_for(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "mp3" => "audio/mp3",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" | "aac" => "audio/aac",
        "ogg" => "audio/ogg",
        "aiff" | "aif" => "audio/aiff",
        _ => "audio/mpeg",
    }
}

fn io_err(e: std::io::Error) -> AppError {
    AppError::Other(format!("Błąd pliku: {e}"))
}

/// SHA-256 pliku (strumieniowo, bez wczytywania całości do pamięci).
pub fn hash_file(path: &Path) -> AppResult<String> {
    let mut f = File::open(path).map_err(io_err)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(io_err)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Kopiuje plik do `<data>/audio/<hash>.<ext>` (jeśli jeszcze go nie ma).
pub fn import(src: &Path, audio_dir: &Path) -> AppResult<ImportedAudio> {
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp3")
        .to_lowercase();
    if !["mp3", "wav", "flac", "m4a", "aac", "ogg", "aiff", "aif"].contains(&ext.as_str()) {
        return Err(AppError::Other(format!(
            "Nieobsługiwany format .{ext}. Użyj MP3, WAV, FLAC, M4A, OGG lub AIFF."
        )));
    }
    let hash = hash_file(src)?;
    std::fs::create_dir_all(audio_dir).map_err(io_err)?;
    let dest: PathBuf = audio_dir.join(format!("{hash}.{ext}"));
    if !dest.exists() {
        let tmp = dest.with_extension("part");
        std::fs::copy(src, &tmp).map_err(io_err)?;
        std::fs::rename(&tmp, &dest).map_err(io_err)?;
    }
    let size = std::fs::metadata(&dest).map_err(io_err)?.len();
    Ok(ImportedAudio {
        path: dest.to_string_lossy().into_owned(),
        hash,
        size,
        mime: mime_for(&ext).to_string(),
        file_name: src
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct Waveform {
    pub peaks: Vec<f32>,
    pub duration_s: f64,
}

/// Dekoduje cały plik i zwraca znormalizowane peaks (0..1) oraz czas trwania.
pub fn waveform(path: &Path) -> AppResult<Waveform> {
    let file = File::open(path).map_err(io_err)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| AppError::Other(format!("Nie udało się odczytać pliku audio: {e}")))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| AppError::Other("Plik nie zawiera ścieżki audio.".into()))?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| AppError::Other("Nieznany kodek audio.".into()))?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|e| AppError::Other(format!("Brak dekodera dla tego pliku: {e}")))?;

    let mut blocks: Vec<f32> = Vec::new();
    let mut cur = 0f32;
    let mut in_block = 0usize;
    let mut total_frames: u64 = 0;
    let mut rate: u32 = 0;
    let mut samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymError::IoError(_)) | Err(SymError::ResetRequired) => break,
            Err(e) => return Err(AppError::Other(format!("Błąd dekodowania: {e}"))),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                let spec = buf.spec();
                rate = spec.rate();
                let ch = spec.channels().count().max(1);
                let frames = buf.frames();
                samples.resize(buf.samples_interleaved(), f32::MID);
                buf.copy_to_slice_interleaved(&mut samples);
                for f in 0..frames {
                    let mut m = 0f32;
                    for c in 0..ch {
                        m = m.max(samples[f * ch + c].abs());
                    }
                    cur = cur.max(m);
                    in_block += 1;
                    if in_block == BLOCK {
                        blocks.push(cur);
                        cur = 0.0;
                        in_block = 0;
                    }
                }
                total_frames += frames as u64;
            }
            Err(SymError::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }
    if in_block > 0 {
        blocks.push(cur);
    }
    if blocks.is_empty() || rate == 0 {
        return Err(AppError::Other(
            "Nie udało się zdekodować dźwięku z pliku.".into(),
        ));
    }
    Ok(Waveform {
        peaks: downsample(&blocks, PEAK_BINS),
        duration_s: total_frames as f64 / rate as f64,
    })
}

/// Zmniejsza do `n` słupków (max z grupy) i normalizuje do 0..1.
pub fn downsample(src: &[f32], n: usize) -> Vec<f32> {
    if src.is_empty() || n == 0 {
        return vec![];
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let a = i * src.len() / n;
        let b = ((i + 1) * src.len() / n).max(a + 1).min(src.len());
        out.push(src[a..b].iter().cloned().fold(0f32, f32::max));
    }
    let max = out.iter().cloned().fold(0f32, f32::max);
    if max > 0.0 {
        for v in &mut out {
            *v /= max;
        }
    }
    out
}

/// Zapis pliku pomocniczy do testów.
#[cfg(test)]
pub fn write_test_wav(path: &Path, secs: f32, rate: u32) {
    use std::io::Write;
    let n = (secs * rate as f32) as u32;
    let mut f = File::create(path).unwrap();
    let data_len = n * 2;
    f.write_all(b"RIFF").unwrap();
    f.write_all(&(36 + data_len).to_le_bytes()).unwrap();
    f.write_all(b"WAVEfmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
    f.write_all(&1u16.to_le_bytes()).unwrap(); // mono
    f.write_all(&rate.to_le_bytes()).unwrap();
    f.write_all(&(rate * 2).to_le_bytes()).unwrap();
    f.write_all(&2u16.to_le_bytes()).unwrap();
    f.write_all(&16u16.to_le_bytes()).unwrap();
    f.write_all(b"data").unwrap();
    f.write_all(&data_len.to_le_bytes()).unwrap();
    for i in 0..n {
        // rosnąca amplituda, żeby peaks były różne
        let amp = (i as f32 / n as f32) * 20000.0;
        let s = (amp * (i as f32 * 0.05).sin()) as i16;
        f.write_all(&s.to_le_bytes()).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_mapping() {
        assert_eq!(mime_for("WAV"), "audio/wav");
        assert_eq!(mime_for("m4a"), "audio/aac");
        assert_eq!(mime_for("xyz"), "audio/mpeg");
    }

    #[test]
    fn downsample_normalizes_and_keeps_peaks() {
        let v = downsample(&[0.1, 0.5, 0.25, 0.2], 2);
        assert_eq!(v, vec![1.0, 0.5]);
    }

    #[test]
    fn import_copies_once_by_hash_and_rejects_bad_ext() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("Master.wav");
        write_test_wav(&src, 0.2, 8000);
        let a = import(&src, &dir.path().join("audio")).unwrap();
        let b = import(&src, &dir.path().join("audio")).unwrap();
        assert_eq!(a.hash, b.hash);
        assert_eq!(a.mime, "audio/wav");
        assert!(a.path.ends_with(&format!("{}.wav", a.hash)));
        let bad = dir.path().join("a.txt");
        std::fs::write(&bad, "x").unwrap();
        assert!(import(&bad, &dir.path().join("audio")).is_err());
    }

    #[test]
    fn waveform_of_wav_has_duration_and_rising_peaks() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.wav");
        write_test_wav(&p, 2.0, 8000);
        let w = waveform(&p).unwrap();
        assert!(
            (w.duration_s - 2.0).abs() < 0.01,
            "duration {}",
            w.duration_s
        );
        assert_eq!(w.peaks.len(), PEAK_BINS);
        assert!(w.peaks[PEAK_BINS - 1] > w.peaks[10]);
        assert!((w.peaks.iter().cloned().fold(0f32, f32::max) - 1.0).abs() < 1e-6);
    }
}
