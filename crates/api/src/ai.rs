//! Minimal provider-agnostic LLM client (OpenAI-compatible) for caption
//! translation. Optional — returns None when unconfigured.

use serde::Deserialize;

use crate::config::Settings;

#[derive(Clone)]
pub struct Translator {
    base: String,
    key: String,
    model: String,
}

impl Translator {
    pub fn from_settings(s: &Settings) -> Option<Self> {
        match (&s.ai_base_url, &s.ai_key) {
            (Some(base), Some(key)) => Some(Translator {
                base: base.trim_end_matches('/').to_string(),
                key: key.clone(),
                model: s
                    .ai_chat_model
                    .clone()
                    .unwrap_or_else(|| "gpt-4o-mini".to_string()),
            }),
            _ => None,
        }
    }

    /// Translate each line into `lang`, preserving order and count. Returns None
    /// on any failure so the caller can surface a clear error.
    pub async fn translate(&self, lines: &[String], lang: &str) -> Option<Vec<String>> {
        let payload = serde_json::to_string(lines).ok()?;
        let system = format!(
            "Translate each string in the given JSON array into {lang}. Respond with ONLY a JSON \
             object {{\"lines\":[...]}} whose array has exactly the same length and order, each \
             element the translation of the corresponding input. Keep proper nouns."
        );
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": payload},
            ],
            "temperature": 0.2,
            "response_format": {"type": "json_object"}
        });

        let resp = reqwest::Client::new()
            .post(format!("{}/chat/completions", self.base))
            .bearer_auth(&self.key)
            .json(&body)
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "translation call failed");
            return None;
        }
        let parsed: ChatResponse = resp.json().await.ok()?;
        let content = parsed.choices.first()?.message.content.clone();
        let out: LinesJson = serde_json::from_str(&content).ok()?;
        (out.lines.len() == lines.len()).then_some(out.lines)
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}
#[derive(Deserialize)]
struct Choice {
    message: Message,
}
#[derive(Deserialize)]
struct Message {
    content: String,
}
#[derive(Deserialize)]
struct LinesJson {
    lines: Vec<String>,
}
