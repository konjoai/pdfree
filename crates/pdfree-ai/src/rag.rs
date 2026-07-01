//! Retrieval-augmented generation: chunk -> embed -> retrieve (Phase 5+).
//!
//! Reuses Kyro (CRAG/Self-RAG/query decomposition) as the retrieval reference,
//! Kohaku for episodic memory across a user's library, and Vectro for embedding
//! quantization to keep the local index small.

use crate::{AiError, Result};

/// Answer a question about a single document via RAG.
pub fn answer(_pdf_bytes: &[u8], _question: &str) -> Result<String> {
    Err(AiError::NotImplemented("rag::answer"))
}
