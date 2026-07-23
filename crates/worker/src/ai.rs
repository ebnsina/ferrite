//! Provider-agnostic LLM access (OpenAI-compatible chat/completions) for AI
//! highlight selection. Optional — callers fall back to a heuristic when unset.

use serde::Deserialize;

use crate::config::Settings;

#[derive(Clone)]
pub struct Chat {
    base: String,
    key: String,
    model: String,
}

/// One highlight window the model (or heuristic) selected.
#[derive(Debug, Clone)]
pub struct Highlight {
    pub start: f64,
    pub end: f64,
    pub title: String,
}

impl Chat {
    pub fn from_settings(s: &Settings) -> Option<Self> {
        match (&s.ai_base_url, &s.ai_key) {
            (Some(base), Some(key)) => Some(Chat {
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

    /// Ask the model for up to `count` highlight windows from a timestamped
    /// transcript. Returns `None` on any failure so the caller can fall back.
    pub async fn select_highlights(&self, transcript: &str, count: u32) -> Option<Vec<Highlight>> {
        let system = format!(
            "You find the {count} best standalone moments in a video transcript to \
             turn into short vertical clips (15-60 seconds each). Each clip should be \
             self-contained and engaging. Respond with ONLY JSON of the form \
             {{\"clips\":[{{\"start\":<seconds>,\"end\":<seconds>,\"title\":\"<short title>\"}}]}}."
        );
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": transcript},
            ],
            "temperature": 0.4,
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
            tracing::warn!(status = %resp.status(), "highlight LLM call failed");
            return None;
        }
        let parsed: ChatResponse = resp.json().await.ok()?;
        let content = parsed.choices.first()?.message.content.clone();
        let clips: ClipsJson = serde_json::from_str(&content).ok()?;
        Some(
            clips
                .clips
                .into_iter()
                .filter(|c| c.end > c.start)
                .map(|c| Highlight {
                    start: c.start,
                    end: c.end,
                    title: c.title,
                })
                .collect(),
        )
    }
}

impl Chat {
    /// Classify a transcript for policy safety. Returns (flagged, categories).
    /// None on any failure (caller treats moderation as unavailable).
    pub async fn moderate(&self, text: &str) -> Option<(bool, Vec<String>)> {
        let clipped: String = text.chars().take(6000).collect();
        let system = "You are a content-safety classifier. Given a video transcript, return ONLY \
             JSON {\"flagged\":bool,\"categories\":[...]} where categories is any of \
             [\"hate\",\"harassment\",\"violence\",\"sexual\",\"self-harm\",\"illegal\"] that \
             clearly apply. Set flagged=true only for serious, unambiguous policy violations.";
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": clipped},
            ],
            "temperature": 0.0,
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
            return None;
        }
        let parsed: ChatResponse = resp.json().await.ok()?;
        let content = parsed.choices.first()?.message.content.clone();
        let m: ModerationJson = serde_json::from_str(&content).ok()?;
        Some((m.flagged, m.categories))
    }
}

#[derive(Deserialize)]
struct ModerationJson {
    #[serde(default)]
    flagged: bool,
    #[serde(default)]
    categories: Vec<String>,
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
struct ClipsJson {
    clips: Vec<ClipJson>,
}
#[derive(Deserialize)]
struct ClipJson {
    start: f64,
    end: f64,
    #[serde(default)]
    title: String,
}
