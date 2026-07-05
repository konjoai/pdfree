//! # pdfree-ai
//!
//! The PDFree AI/ML layer. **Design principle: local-first, cloud-optional.**
//!
//! PDFree's whole pitch is honesty and privacy, so AI features must honor it:
//!
//! - Default to on-device models so documents never leave the machine.
//! - Offer cloud providers (Claude, GPT, Gemini) as an explicit opt-in.
//! - Every AI action states where processing happens — no silent uploads.
//!
//! The [`provider`] module defines a trait-based abstraction so any feature can
//! run against a local or cloud backend interchangeably. Feature modules
//! ([`rag`], [`ocr`], [`redact`], [`extract`], [`classify`]) are scaffolded for
//! Phases 5–7 and currently return [`AiError::NotImplemented`]. [`confidence`]
//! is the exception — a Phase 5 quick win that needs no provider at all, so
//! it's already fully implemented: a plain grounding check any of the other
//! modules can run on a value before showing it to the user.

pub mod classify;
pub mod confidence;
pub mod extract;
pub mod ocr;
pub mod provider;
pub mod rag;
pub mod redact;

/// Result alias for the AI layer.
pub type Result<T> = std::result::Result<T, AiError>;

/// Errors from the AI layer.
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    /// Underlying PDF engine error.
    #[error(transparent)]
    Core(#[from] pdfree_core::PdfError),

    /// A provider (local or cloud) failed.
    #[error("AI provider error: {0}")]
    Provider(String),

    /// A feature is scaffolded but not implemented yet.
    #[error("`{0}` is not implemented yet")]
    NotImplemented(&'static str),
}
