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
