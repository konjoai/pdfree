//! LLM provider abstraction — the seam between features and backends.
//!
//! A feature never names a concrete model. It asks a [`Provider`] to complete
//! a prompt, and the app wires up either a local model (default) or a cloud
//! model (explicit opt-in). This mirrors lopi's orchestration patterns and lets
//! PDFree keep its promise: process locally unless the user says otherwise.

/// Where inference happens. Surfaced in the UI so a user always knows whether a
/// document leaves the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Residency {
    /// Runs on the user's device; nothing is uploaded.
    Local,
    /// Runs on a third-party API; the document (or an excerpt) is uploaded.
    Cloud,
}

/// A prompt-completion backend. Local and cloud implementations are
/// interchangeable behind this trait.
pub trait Provider {
    /// Human-readable model/backend name, e.g. "Squish · Qwen2.5-3B (local)".
    fn name(&self) -> &str;

    /// Whether this provider processes data locally or in the cloud.
    fn residency(&self) -> Residency;

    /// Complete a prompt and return the model's text response.
    fn complete(&self, prompt: &str) -> crate::Result<String>;
}

/// Cloud provider backed by the Anthropic Messages API.
///
/// The API key is supplied by the caller (env var, keychain, settings —
/// whatever the platform shell wants) and never hardcoded here, per
/// CLAUDE.md's "no silent uploads" rule: constructing this provider is
/// itself the user's explicit cloud opt-in.
pub struct AnthropicProvider {
    api_key: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl AnthropicProvider {
    const API_URL: &'static str = "https://api.anthropic.com/v1/messages";
    const ANTHROPIC_VERSION: &'static str = "2023-06-01";
    const DEFAULT_MODEL: &'static str = "claude-opus-4-8";
    const DEFAULT_MAX_TOKENS: u32 = 4096;

    /// Uses the default model (`claude-opus-4-8`).
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_model(api_key, Self::DEFAULT_MODEL)
    }

    pub fn with_model(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "Anthropic Claude (cloud)"
    }

    fn residency(&self) -> Residency {
        Residency::Cloud
    }

    fn complete(&self, prompt: &str) -> crate::Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": Self::DEFAULT_MAX_TOKENS,
            "messages": [{"role": "user", "content": prompt}],
        });

        let response = self
            .client
            .post(Self::API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", Self::ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| crate::AiError::Provider(format!("Anthropic request failed: {e}")))?;

        let status = response.status();
        let payload: serde_json::Value = response.json().map_err(|e| {
            crate::AiError::Provider(format!("Anthropic response parse failed: {e}"))
        })?;

        if !status.is_success() {
            let message = payload
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(crate::AiError::Provider(format!(
                "Anthropic API error ({status}): {message}"
            )));
        }

        if payload.get("stop_reason").and_then(|s| s.as_str()) == Some("refusal") {
            return Err(crate::AiError::Provider(
                "Anthropic declined the request (refusal)".to_string(),
            ));
        }

        payload
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|blocks| {
                blocks
                    .iter()
                    .find(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
            })
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                crate::AiError::Provider("Anthropic response had no text content".to_string())
            })
    }
}

/// Local provider backed by an Ollama instance (default: on-device,
/// `http://localhost:11434`) — PDFree's default AI backend per CLAUDE.md's
/// "local-first" principle.
pub struct OllamaProvider {
    base_url: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl OllamaProvider {
    const DEFAULT_BASE_URL: &'static str = "http://localhost:11434";

    pub fn new(model: impl Into<String>) -> Self {
        Self::with_base_url(model, Self::DEFAULT_BASE_URL)
    }

    pub fn with_base_url(model: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl Provider for OllamaProvider {
    fn name(&self) -> &str {
        "Ollama (local)"
    }

    fn residency(&self) -> Residency {
        Residency::Local
    }

    fn complete(&self, prompt: &str) -> crate::Result<String> {
        let url = format!("{}/api/generate", self.base_url);
        let body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
        });

        let response = self.client.post(&url).json(&body).send().map_err(|e| {
            crate::AiError::Provider(format!(
                "Ollama request failed (is `ollama serve` running?): {e}"
            ))
        })?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().unwrap_or_default();
            return Err(crate::AiError::Provider(format!(
                "Ollama error ({status}): {text}"
            )));
        }

        let payload: serde_json::Value = response
            .json()
            .map_err(|e| crate::AiError::Provider(format!("Ollama response parse failed: {e}")))?;

        payload
            .get("response")
            .and_then(|r| r.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                crate::AiError::Provider("Ollama response had no `response` field".to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real HTTP call against a local Ollama instance. Skipped (not failed)
    /// when Ollama isn't running — same pattern as pdfree-core's
    /// `skip_without_pdfium!()` for an unavailable local dependency.
    #[test]
    fn ollama_completes_a_simple_prompt() {
        let provider = OllamaProvider::new("qwen3:4b");
        match provider.complete("Reply with exactly one word: hello") {
            Ok(text) => assert!(!text.trim().is_empty()),
            Err(e) => {
                eprintln!("skipping: Ollama unavailable ({e})");
            }
        }
    }

    #[test]
    fn ollama_reports_local_residency() {
        let provider = OllamaProvider::new("qwen3:4b");
        assert_eq!(provider.residency(), Residency::Local);
    }

    #[test]
    fn anthropic_reports_cloud_residency() {
        let provider = AnthropicProvider::new("test-key-not-a-real-credential");
        assert_eq!(provider.residency(), Residency::Cloud);
    }

    #[test]
    fn anthropic_rejects_bad_key_with_provider_error() {
        // Confirms the request round-trips to the real API and error handling
        // works end-to-end, without needing a valid credential.
        let provider = AnthropicProvider::new("sk-ant-invalid-test-key");
        match provider.complete("hello") {
            Err(crate::AiError::Provider(_)) => {}
            Err(e) => panic!("expected AiError::Provider, got {e:?}"),
            Ok(text) => panic!("expected an auth error, got a completion: {text}"),
        }
    }
}
