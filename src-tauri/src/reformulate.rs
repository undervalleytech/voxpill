//! Reformulation post-transcription via API compatible OpenAI (Groq, OpenRouter).

use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;

const GROQ_URL: &str = "https://api.groq.com/openai/v1/chat/completions";
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Règles de sortie toujours appliquées après le prompt personnalisé (évite préambules du type « Voici… »).
const OUTPUT_CONTRACT: &str = r#"Output contract (always follow, in addition to the instructions above):
- Return ONLY the rewritten transcript as plain text — one continuous block, nothing else.
- Keep EXACTLY the same language as the input transcript. Never translate.
- Do NOT write any introduction, preamble, title, label, or explanation (e.g. no "Here is", "Voici", "Sure", "Below is", "The reformulation is", or similar).
- Do NOT describe what you did or repeat the task; do not answer in a conversational tone.
- Do NOT wrap the entire answer in quotation marks unless the original transcript clearly required them.
- If the transcript is empty or whitespace, return nothing (empty response)."#;

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Option<Vec<Choice>>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<MessageBody>,
}

#[derive(Debug, Deserialize)]
struct MessageBody {
    content: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ReformulationProfileSnap {
    pub model: String,
    pub prompt: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ReformulationParams {
    pub enabled: bool,
    pub api_key: String,
    pub provider: String,
    pub active_profile: u8,
    pub profiles: Vec<ReformulationProfileSnap>,
    /// Non vide : utilisé uniquement par `assist_from_instruction` à la place du modèle du profil actif.
    pub assist_model_override: String,
}

/// Retourne le texte reformulé, ou `None` si désactivé / clé absente / erreur (fallback côté appelant).
pub(crate) fn maybe_reformulate(params: &ReformulationParams, user_text: &str) -> Option<String> {
    if !params.enabled {
        return None;
    }
    let key = params.api_key.trim();
    if key.is_empty() {
        return None;
    }
    let provider = params.provider.trim().to_ascii_lowercase();
    if provider != "groq" && provider != "openrouter" {
        eprintln!("[Voxpill] Reformulation : fournisseur inconnu « {provider} »");
        return None;
    }
    let idx = params.active_profile.saturating_sub(1) as usize;
    let p = params.profiles.get(idx)?;
    let model = p.model.trim();
    if model.is_empty() {
        eprintln!("[Voxpill] Reformulation : modèle vide pour le profil {}", idx + 1);
        return None;
    }
    let system_final = build_system_content(p.prompt.trim());

    let url = if provider == "groq" {
        GROQ_URL
    } else {
        OPENROUTER_URL
    };

    let client = match Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[Voxpill] Reformulation : client HTTP : {e}");
            return None;
        }
    };

    let body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_final},
            {"role": "user", "content": format!(
                "Rewrite the transcript below in the same language as the transcript (do not translate). Reply with the rewritten text only — no other words before or after.\n\n---\n{}\n---",
                user_text
            )}
        ],
        "temperature": 0.15,
        "max_tokens": 2048,
    });

    let mut req = client.post(url).header("Authorization", format!("Bearer {key}"));
    if provider == "openrouter" {
        req = req
            .header("HTTP-Referer", "https://github.com/voxpill/voxpill")
            .header("X-Title", "Voxpill");
    }
    let req = req.json(&body);

    let resp = match req.send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[Voxpill] Reformulation : requête : {e}");
            return None;
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        eprintln!("[Voxpill] Reformulation : HTTP {status} — {text}");
        return None;
    }

    let parsed: ChatCompletionResponse = match resp.json() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[Voxpill] Reformulation : JSON : {e}");
            return None;
        }
    };

    let out = parsed
        .choices?
        .first()?
        .message
        .as_ref()?
        .content
        .as_ref()?
        .trim()
        .to_string();

    let out = strip_common_preamble(&out);

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Réponse générée à partir d’une consigne vocale (transcription = demande utilisateur).
/// `selected_text` : texte copié depuis l’app au début de l’enregistrement assist (optionnel).
/// Réutilise clé API, fournisseur et modèle du profil actif. Pas de reformulation « rewrite ».
pub(crate) fn assist_from_instruction(
    params: &ReformulationParams,
    selected_text: Option<&str>,
    user_instruction: &str,
) -> Option<String> {
    let key = params.api_key.trim();
    if key.is_empty() {
        eprintln!("[Voxpill] Assist : clé API absente");
        return None;
    }
    let provider = params.provider.trim().to_ascii_lowercase();
    if provider != "groq" && provider != "openrouter" {
        eprintln!("[Voxpill] Assist : fournisseur inconnu « {provider} »");
        return None;
    }
    let idx = params.active_profile.saturating_sub(1) as usize;
    let p = params.profiles.get(idx)?;
    let override_trim = params.assist_model_override.trim();
    let model = if override_trim.is_empty() {
        p.model.trim()
    } else {
        override_trim
    };
    if model.is_empty() {
        eprintln!("[Voxpill] Assist : modèle vide (profil {} ou override assist)", idx + 1);
        return None;
    }
    let user_instruction = user_instruction.trim();
    if user_instruction.is_empty() {
        return None;
    }

    const ASSIST_SYSTEM: &str = r#"You are a concise assistant. The user spoke a request (possibly via speech-to-text).
Fulfill the request in the same language as the input when relevant. Output only the answer or content — no preamble, no title, no "Here is", no meta-commentary."#;

    const ASSIST_SYSTEM_WITH_SELECTION: &str = r#"You are a concise assistant. The user message has two parts: "Selected text" (content from their editor) and "Spoken instruction" (what they want).
The selected text is your primary context: names, lists, paragraphs, and facts needed for the task are there. Read it carefully and use it as the subject of the instruction. Do not ask the user to repeat names or details that already appear in the selected text.
Fulfill the spoken instruction using that context. If the instruction asks to add details (dates, clubs, facts, etc.), apply them to the people or items named in the selected text.
When the instruction is only to change, replace, or fix a specific part of the text, keep the rest unchanged (minimal edit). When the instruction is to expand or enrich the whole selection, output the full updated text as requested.
Match the language of the instruction and text when relevant. Output only the result — no preamble, no title, no "Here is", no meta-commentary."#;

    let (system_content, user_content) = match selected_text {
        Some(s) if !s.trim().is_empty() => (
            ASSIST_SYSTEM_WITH_SELECTION,
            format!(
                "Selected text:\n{}\n\nSpoken instruction:\n{}",
                s.trim(),
                user_instruction
            ),
        ),
        _ => (ASSIST_SYSTEM, user_instruction.to_string()),
    };

    match selected_text {
        Some(s) if !s.trim().is_empty() => {
            let n = s.chars().count();
            eprintln!(
                "[Voxpill] Assist LLM : requête avec texte sélectionné ({n} car.) + consigne ({} car.)",
                user_instruction.chars().count()
            );
        }
        _ => {
            eprintln!(
                "[Voxpill] Assist LLM : requête sans sélection — consigne seule ({} car.)",
                user_instruction.chars().count()
            );
        }
    }

    let url = if provider == "groq" {
        GROQ_URL
    } else {
        OPENROUTER_URL
    };

    let client = match Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[Voxpill] Assist : client HTTP : {e}");
            return None;
        }
    };

    let body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_content},
            {"role": "user", "content": user_content}
        ],
        "temperature": 0.35,
        "max_tokens": 4096,
    });

    let mut req = client.post(url).header("Authorization", format!("Bearer {key}"));
    if provider == "openrouter" {
        req = req
            .header("HTTP-Referer", "https://github.com/voxpill/voxpill")
            .header("X-Title", "Voxpill");
    }
    let req = req.json(&body);

    let resp = match req.send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[Voxpill] Assist : requête : {e}");
            return None;
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        eprintln!("[Voxpill] Assist : HTTP {status} — {text}");
        return None;
    }

    let parsed: ChatCompletionResponse = match resp.json() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[Voxpill] Assist : JSON : {e}");
            return None;
        }
    };

    let out = parsed
        .choices?
        .first()?
        .message
        .as_ref()?
        .content
        .as_ref()?
        .trim()
        .to_string();

    let out = strip_common_preamble(&out);

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn build_system_content(user_prompt: &str) -> String {
    let base = if user_prompt.is_empty() {
        default_system_fallback()
    } else {
        user_prompt.to_string()
    };
    format!("{}\n\n{}", base, OUTPUT_CONTRACT)
}

fn default_system_fallback() -> String {
    "You rewrite dictated text for clarity (e.g. for software / IDE use when relevant). Preserve meaning and language; do not invent facts, steps, or code.".to_string()
}

/// Filet de sécurité si le modèle ajoute encore « Voici … : » / « Here is …: » avant le vrai texte.
fn strip_common_preamble(s: &str) -> String {
    let t = s.trim();
    let low = t.to_ascii_lowercase();
    let looks_meta = low.starts_with("voici")
        || low.starts_with("here is")
        || low.starts_with("here's")
        || low.starts_with("sure,")
        || low.starts_with("sure ")
        || low.starts_with("below is")
        || low.starts_with("the reformulation");
    if looks_meta {
        if let Some(pos) = t.find(':') {
            let after = t[pos + 1..]
                .trim()
                .trim_start_matches(|c| c == '"' || c == '«' || c == '\'');
            // Garder la suite seulement si elle ressemble au contenu (évite faux positifs sur texte court)
            if !after.is_empty() && after.len() + 8 < t.len() {
                return after.to_string();
            }
        }
    }
    t.to_string()
}

pub(crate) fn assist_prompt(
    provider: &str,
    api_key: &str,
    draft: &str,
    model_hint: &str,
) -> Result<String, String> {
    let provider = provider.trim().to_ascii_lowercase();
    if provider != "groq" && provider != "openrouter" {
        return Err("Unsupported provider. Use Groq or OpenRouter.".to_string());
    }
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("Missing API key. Add your API key first.".to_string());
    }
    let draft = draft.trim();
    if draft.is_empty() {
        return Err("Write a draft prompt first, then click Refine prompt.".to_string());
    }

    let system = r#"You improve user-written prompt drafts for speech-transcript rewriting.
Return only the final prompt text.
Do not add markdown, quotes, headings, or explanations.
Keep it concise, practical, and robust.
If the user draft is vague, infer a complete, high-quality prompt from intent.
The prompt must enforce:
- preserve original meaning and language
- no invented facts, steps, or code
- output only rewritten text (no preamble)
- keep the same language as the dictated transcript (never translate)
- preserve technical terms and proper names"#;
    let user = format!(
        "Rewrite this draft into a strong production-ready prompt:\n\n---\n{}\n---",
        draft
    );

    let model = if model_hint.trim().is_empty() {
        if provider == "groq" {
            "llama-3.1-8b-instant"
        } else {
            "meta-llama/llama-3.1-8b-instruct"
        }
    } else {
        model_hint.trim()
    };

    call_chat_completion(&provider, api_key, model, &user, system, 0.2, 600)
}

fn call_chat_completion(
    provider: &str,
    api_key: &str,
    default_model: &str,
    user_content: &str,
    system_content: &str,
    temperature: f32,
    max_tokens: u32,
) -> Result<String, String> {
    let url = if provider == "groq" {
        GROQ_URL
    } else {
        OPENROUTER_URL
    };
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let body = json!({
        "model": default_model,
        "messages": [
            {"role": "system", "content": system_content},
            {"role": "user", "content": user_content}
        ],
        "temperature": temperature,
        "max_tokens": max_tokens,
    });
    let mut req = client
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"));
    if provider == "openrouter" {
        req = req
            .header("HTTP-Referer", "https://github.com/voxpill/voxpill")
            .header("X-Title", "Voxpill");
    }

    let resp = req
        .json(&body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("HTTP {status}: {text}"));
    }
    let parsed: ChatCompletionResponse = resp
        .json()
        .map_err(|e| format!("Invalid provider response: {e}"))?;
    let out = parsed
        .choices
        .ok_or_else(|| "Missing choices in provider response.".to_string())?
        .first()
        .and_then(|c| c.message.as_ref())
        .and_then(|m| m.content.as_ref())
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "Provider returned an empty answer.".to_string())?;
    if out.is_empty() {
        Err("Provider returned an empty answer.".to_string())
    } else {
        Ok(out)
    }
}
