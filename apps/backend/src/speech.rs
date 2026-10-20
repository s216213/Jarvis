//! Local voice engine on the OCI compose network: Faster-Whisper in, Kokoro out.
//! Neither container publishes a host port. Text chat does not use this module.

use std::sync::OnceLock;
use std::time::Duration;

use axum::extract::Multipart;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;

use crate::error::{AppError, AppResult};

pub const KOKORO_VOICES: &[&str] = &[
    "bm_george",
    "bm_daniel",
    "bm_fable",
    "am_adam",
    "am_michael",
];

const DEFAULT_VOICE: &str = "bm_george";
const MAX_AUDIO_BYTES: usize = 700_000;
const MAX_SECONDS: f64 = 20.0;

pub struct VoiceUpload {
    pub wav: Vec<u8>,
    pub voice: String,
    pub device_fingerprint: Option<String>,
}

pub fn resolve_voice(raw: Option<&str>) -> AppResult<String> {
    let voice = raw.map(str::trim).filter(|s| !s.is_empty()).unwrap_or(DEFAULT_VOICE);
    if KOKORO_VOICES.contains(&voice) {
        Ok(voice.to_string())
    } else {
        Err(AppError::BadRequest(format!("unknown voice {voice}")))
    }
}

pub async fn read_upload(mut multipart: Multipart) -> AppResult<VoiceUpload> {
    let mut wav = None;
    let mut voice = None;
    let mut device_fingerprint = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("invalid upload: {e}")))?
    {
        match field.name() {
            Some("audio") => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("invalid audio: {e}")))?;
                if bytes.len() > MAX_AUDIO_BYTES {
                    return Err(AppError::BadRequest(
                        "audio exceeds 20 seconds".into(),
                    ));
                }
                wav = Some(bytes.to_vec());
            }
            Some("device_fingerprint") => {
                device_fingerprint = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::BadRequest(format!("invalid device: {e}")))?,
                );
            }
            Some("voice") => {
                voice = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::BadRequest(format!("invalid voice: {e}")))?,
                );
            }
            _ => {}
        }
    }

    let wav = wav.ok_or_else(|| AppError::BadRequest("audio is required".into()))?;
    validate_wav(&wav)?;
    Ok(VoiceUpload {
        wav,
        voice: resolve_voice(voice.as_deref())?,
        device_fingerprint: device_fingerprint
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    })
}

pub fn validate_wav(bytes: &[u8]) -> AppResult<()> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(AppError::BadRequest(
            "audio must be 16 kHz mono 16-bit PCM WAV".into(),
        ));
    }

    let mut offset = 12usize;
    let mut format = 0u16;
    let mut channels = 0u16;
    let mut rate = 0u32;
    let mut bits = 0u16;
    let mut data_len = 0u64;
    let mut saw_fmt = false;

    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let start = offset + 8;
        if start + size > bytes.len() {
            return Err(AppError::BadRequest(
                "audio must be 16 kHz mono 16-bit PCM WAV".into(),
            ));
        }
        if id == b"fmt " && size >= 16 {
            format = u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap());
            channels = u16::from_le_bytes(bytes[start + 2..start + 4].try_into().unwrap());
            rate = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap());
            bits = u16::from_le_bytes(bytes[start + 14..start + 16].try_into().unwrap());
            saw_fmt = true;
        } else if id == b"data" {
            data_len = data_len.saturating_add(size as u64);
        }
        let padded = size + (size % 2);
        offset = start + padded;
    }

    if !saw_fmt || format != 1 || channels != 1 || rate != 16_000 || bits != 16 || data_len == 0 {
        return Err(AppError::BadRequest(
            "audio must be 16 kHz mono 16-bit PCM WAV".into(),
        ));
    }

    let seconds = data_len as f64 / (16_000.0 * 2.0);
    if seconds > MAX_SECONDS + 0.05 {
        return Err(AppError::BadRequest("audio exceeds 20 seconds".into()));
    }
    Ok(())
}

pub async fn transcribe(base: &str, wav: &[u8]) -> AppResult<String> {
    let part = reqwest::multipart::Part::bytes(wav.to_vec())
        .file_name("speech.wav")
        .mime_str("audio/wav")
        .map_err(|e| AppError::internal(format!("wav part: {e}")))?;
    let form = reqwest::multipart::Form::new().part("audio_file", part);
    let url = format!(
        "{base}/asr?task=transcribe&language=pl&output=json&vad_filter=true"
    );
    let response = client()
        .post(url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| AppError::Upstream(format!("speech service unavailable: {e}")))?;

    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|e| AppError::Upstream(format!("speech service unavailable: {e}")))?;
    if !status.is_success() {
        return Err(AppError::Upstream(format!(
            "speech service unavailable: HTTP {status}"
        )));
    }

    let text = transcript_text(&body)?;
    if text.is_empty() {
        return Err(AppError::BadRequest("no speech recognized".into()));
    }
    Ok(text)
}

fn transcript_text(body: &[u8]) -> AppResult<String> {
    if let Ok(parsed) = serde_json::from_slice::<AsrJson>(body) {
        return Ok(parsed.text.unwrap_or_default().trim().to_string());
    }
    let raw = String::from_utf8_lossy(body).trim().to_string();
    if raw.starts_with('{') {
        return Err(AppError::Upstream(
            "speech service unavailable: unexpected transcript".into(),
        ));
    }
    Ok(raw)
}

#[derive(Deserialize)]
struct AsrJson {
    text: Option<String>,
}

/// Turn markdown into words a person would say. Lists become "First… Second… And last…".
pub fn speakable(text: &str) -> String {
    let mut parts = Vec::new();
    let mut items = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("```") {
            flush_items(&mut items, &mut parts);
            continue;
        }
        if let Some(item) = list_item(trimmed) {
            let spoken = plain(&item);
            if !spoken.is_empty() {
                items.push(spoken);
            }
            continue;
        }
        flush_items(&mut items, &mut parts);
        let spoken = finish(plain(trimmed.trim_start_matches('#').trim()));
        if !spoken.is_empty() {
            parts.push(spoken);
        }
    }
    flush_items(&mut items, &mut parts);
    let spoken = parts.join(" ");
    if spoken.is_empty() {
        text.trim().to_string()
    } else {
        spoken
    }
}

fn flush_items(items: &mut Vec<String>, parts: &mut Vec<String>) {
    if items.is_empty() {
        return;
    }
    parts.push(speak_items(items));
    items.clear();
}

fn speak_items(items: &[String]) -> String {
    if items.len() == 1 {
        return finish(items[0].clone());
    }
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let body = item.trim().trim_end_matches(['.', ';']);
            format!("{}, {body}.", item_prefix(index, items.len()))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn item_prefix(index: usize, len: usize) -> &'static str {
    const ORDS: [&str; 10] = [
        "First", "Second", "Third", "Fourth", "Fifth", "Sixth", "Seventh", "Eighth", "Ninth",
        "Tenth",
    ];
    if index + 1 == len {
        if len == 2 { "And second" } else { "And last" }
    } else if index < ORDS.len() {
        ORDS[index]
    } else {
        "Next"
    }
}

fn list_item(line: &str) -> Option<String> {
    let rest = line.trim_start();
    if let Some(item) = rest
        .strip_prefix("- ")
        .or_else(|| rest.strip_prefix("* "))
        .or_else(|| rest.strip_prefix("+ "))
    {
        return Some(item.to_string());
    }
    let digits = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    rest[digits..]
        .strip_prefix(". ")
        .or_else(|| rest[digits..].strip_prefix(") "))
        .map(str::to_string)
}

fn plain(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '['
            && let Some(label_end) = chars[index + 1..].iter().position(|ch| *ch == ']')
        {
            let close = index + 1 + label_end;
            if close + 1 < chars.len() && chars[close + 1] == '('
                && let Some(url_end) = chars[close + 2..].iter().position(|ch| *ch == ')')
            {
                out.extend(chars[index + 1..close].iter());
                index = close + 2 + url_end + 1;
                continue;
            }
        }
        if matches!(chars[index], '`' | '*' | '_' | '#') {
            index += 1;
            continue;
        }
        out.push(chars[index]);
        index += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn finish(text: String) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.ends_with(['.', '!', '?', ':', ';']) {
        trimmed.to_string()
    } else {
        format!("{trimmed}.")
    }
}

pub async fn synthesize(base: &str, text: &str, voice: &str) -> AppResult<Vec<u8>> {
    let response = client()
        .post(format!("{base}/v1/audio/speech"))
        .json(&serde_json::json!({
            "model": "kokoro",
            "input": speakable(text),
            "voice": voice,
            "response_format": "wav",
            "speed": 1.0,
        }))
        .send()
        .await
        .map_err(|e| AppError::Upstream(format!("speech synthesis unavailable: {e}")))?;

    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::Upstream(format!("speech synthesis unavailable: {e}")))?;
    if !status.is_success() {
        return Err(AppError::Upstream(format!(
            "speech synthesis unavailable: HTTP {status}"
        )));
    }
    if bytes.len() < 44 || &bytes[..4] != b"RIFF" {
        return Err(AppError::Upstream(
            "speech synthesis unavailable: response was not WAV".into(),
        ));
    }
    Ok(bytes.to_vec())
}

pub fn encode_wav(wav: &[u8]) -> String {
    STANDARD.encode(wav)
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("speech http client")
    })
}

#[cfg(test)]
mod tests {
    use super::{resolve_voice, speakable, transcript_text, validate_wav};

    fn pcm_wav(samples: u32) -> Vec<u8> {
        let data_len = samples * 2;
        let mut bytes = vec![0u8; 44 + data_len as usize];
        bytes[0..4].copy_from_slice(b"RIFF");
        bytes[4..8].copy_from_slice(&(36 + data_len).to_le_bytes());
        bytes[8..12].copy_from_slice(b"WAVE");
        bytes[12..16].copy_from_slice(b"fmt ");
        bytes[16..20].copy_from_slice(&16u32.to_le_bytes());
        bytes[20..22].copy_from_slice(&1u16.to_le_bytes());
        bytes[22..24].copy_from_slice(&1u16.to_le_bytes());
        bytes[24..28].copy_from_slice(&16_000u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&(16_000u32 * 2).to_le_bytes());
        bytes[32..34].copy_from_slice(&2u16.to_le_bytes());
        bytes[34..36].copy_from_slice(&16u16.to_le_bytes());
        bytes[36..40].copy_from_slice(b"data");
        bytes[40..44].copy_from_slice(&data_len.to_le_bytes());
        bytes
    }

    #[test]
    fn accepts_a_short_pcm_wav() {
        validate_wav(&pcm_wav(16_000)).unwrap();
    }

    #[test]
    fn rejects_audio_longer_than_twenty_seconds() {
        let err = validate_wav(&pcm_wav(16_000 * 21)).unwrap_err();
        assert!(err.to_string().contains("20 seconds"));
    }

    #[test]
    fn rejects_stereo() {
        let mut wav = pcm_wav(800);
        wav[22..24].copy_from_slice(&2u16.to_le_bytes());
        assert!(validate_wav(&wav).is_err());
    }

    #[test]
    fn voice_defaults_to_george() {
        assert_eq!(resolve_voice(None).unwrap(), "bm_george");
        assert!(resolve_voice(Some("alloy")).is_err());
    }

    #[test]
    fn lists_are_read_as_spoken_items() {
        let text = speakable("Bring these:\n- milk\n- bread\n- eggs");
        assert_eq!(
            text,
            "Bring these: First, milk. Second, bread. And last, eggs."
        );
    }

    #[test]
    fn two_numbered_items_use_first_and_second() {
        let text = speakable("1. Alpha\n2. Beta");
        assert_eq!(text, "First, Alpha. And second, Beta.");
    }

    #[test]
    fn prose_keeps_its_sentences() {
        let text = speakable("Yes. Use **notepad** today.");
        assert_eq!(text, "Yes. Use notepad today.");
    }

    #[test]
    fn transcript_uses_the_json_text_field() {
        let text = transcript_text(br#"{"text":"  czesc  ","language":"pl"}"#).unwrap();
        assert_eq!(text, "czesc");
    }
}
