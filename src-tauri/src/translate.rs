//! Traduction post-dictée via l’API publique MyMemory (gratuite, sans clé ; limites d’usage).

use reqwest::blocking::Client;

fn translate_chunk(client: &Client, text: &str, source: &str, target: &str) -> Result<String, String> {
    let url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair={}|{}",
        urlencoding::encode(text),
        source,
        target
    );
    let resp = client.get(&url).send().map_err(|e| e.to_string())?;
    let v: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    let s = v["responseData"]["translatedText"]
        .as_str()
        .ok_or_else(|| "Réponse de traduction invalide".to_string())?;
    if s.contains("MYMEMORY WARNING") {
        return Err("Limite de traduction atteinte (MyMemory). Réessayez plus tard.".to_string());
    }
    Ok(s.to_string())
}

/// Traduit `text` depuis `source` vers `target` (codes ISO type whisper : fr, en, de…).
pub fn translate_mymemory(text: &str, source: &str, target: &str) -> Result<String, String> {
    if text.is_empty() {
        return Ok(String::new());
    }
    let source = source.trim();
    let target = target.trim();
    if source.eq_ignore_ascii_case(target) {
        return Ok(text.to_string());
    }
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    const MAX: usize = 420;
    if text.len() <= MAX {
        return translate_chunk(&client, text, source, target);
    }
    let mut out = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        let take = if rest.len() <= MAX {
            rest.len()
        } else {
            let mut cut = MAX;
            while cut > 0 && !rest.is_char_boundary(cut) {
                cut -= 1;
            }
            if let Some(pos) = rest[..cut].rfind(|c: char| c.is_whitespace()) {
                (pos + 1).max(1)
            } else {
                cut.max(1)
            }
        };
        let (chunk, rem) = rest.split_at(take);
        let piece = chunk.trim();
        if !piece.is_empty() {
            let t = translate_chunk(&client, piece, source, target)?;
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&t);
        }
        rest = rem.trim_start();
    }
    Ok(out)
}
