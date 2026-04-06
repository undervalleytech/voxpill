//! Décodage fichier audio → PCM f32 mono (Symphonia : WAV, MP3, FLAC, OGG, AAC/M4A, etc.).
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphErr;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Décode tout le fichier en échantillons f32 **entrelacés** (canaux), puis mixe en mono.
pub fn decode_file_to_mono_f32(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let file = File::open(path).map_err(|e| format!("Ouverture fichier : {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("Format audio non reconnu ou fichier illisible : {e}"))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| "Aucune piste audio exploitable.".to_string())?;

    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let rate = codec_params
        .sample_rate
        .ok_or_else(|| "Fréquence d’échantillonnage inconnue.".to_string())?;
    let channels = codec_params.channels.map(|c| c.count()).unwrap_or(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Codec non supporté : {e}"))?;

    let mut samples_out: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphErr::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(format!("Lecture audio : {e}")),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                append_decoded_f32(decoded, &mut samples_out)?;
            }
            Err(SymphErr::DecodeError(_)) => continue,
            Err(e) => return Err(format!("Décodage : {e}")),
        }
    }

    if samples_out.is_empty() {
        return Err("Aucun échantillon audio décodé.".to_string());
    }

    let mono = if channels <= 1 {
        samples_out
    } else {
        samples_out
            .chunks(channels as usize)
            .map(|ch| ch.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    Ok((mono, rate))
}

fn append_decoded_f32(decoded: AudioBufferRef, out: &mut Vec<f32>) -> Result<(), String> {
    let spec = *decoded.spec();
    let duration = decoded.capacity() as u64;
    let mut buf = SampleBuffer::<f32>::new(duration, spec);
    buf.copy_interleaved_ref(decoded);
    out.extend_from_slice(buf.samples());
    Ok(())
}
