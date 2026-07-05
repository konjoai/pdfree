# PDFree AI Design

**Design principle: local-first, cloud-optional.** PDFree's pitch is honesty and
privacy, so AI must honor it:

- Default to on-device models (Squish runtime on Apple Silicon; small quantized
  models elsewhere) so documents never leave the machine.
- Offer cloud providers (Claude, GPT, Gemini) as an explicit opt-in.
- Every AI action states where processing happens. **No silent uploads.**

## Provider abstraction (`pdfree-ai/src/provider.rs`)

A trait-based seam so any feature runs against a local or cloud backend
interchangeably (reuses lopi's orchestration patterns; Squish for local
inference). Each provider reports its [`Residency`] (`Local` / `Cloud`), which the
UI surfaces so the user always knows whether a document leaves the device.

```rust
pub trait Provider {
    fn name(&self) -> &str;
    fn residency(&self) -> Residency;   // Local | Cloud
    fn complete(&self, prompt: &str) -> Result<String>;
}
```

## Tiers

### Tier 1 — Core AI (ship with v1 / fast-follow v1.1)
| Feature | Module | Konjo synergy |
|---|---|---|
| Document Q&A / chat (RAG over a doc) | `rag.rs` | lopi + Kyro RAG + Kohaku |
| Smart form fill (detect fields, suggest from profile) | `extract.rs` | — |
| OCR + LLM cleanup | `ocr.rs` | Squish local inference |
| Auto-summary (TL;DR) | `provider.rs` | Squish |

### Tier 2 — Differentiators
| Feature | Module | Konjo synergy |
|---|---|---|
| Smart redaction (PII auto-detect) | `redact.rs` | Squash compliance |
| Contract analysis (risky clauses, legalese → plain) | `extract.rs` | Squash |
| Table extraction → CSV/Excel/JSON | `extract.rs` | Vectro |
| Semantic search across library | `rag.rs` | Vectro + Kohaku |
| Auto-classify (contract/invoice/tax/receipt) | `classify.rs` | Vectro |

### Tier 3 — Expansion (v2+)
Layout-aware translation; layout-aware editing; voice-to-fill; grammar/tone
rewrite; schema-driven extraction; document diff/redline; agentic document
workflows (lopi); review routing.

### Confidence scoring (`confidence.rs`) — already implemented

Pulled forward from Tier 3 by the 2026-07-03 feature research pass: unlike
every other module above, grounding a value against the source document's
own text is a plain string search, not a model call, so it needs no
provider and no open decision to ship. `confidence::ground_check(document_text,
value)` reports `Grounding::Exact` / `Partial` / `Ungrounded` (plus a
matching `0.0..=1.0` confidence and, for an exact match, a short context
excerpt) — case-insensitive and whitespace-normalized so PDF text
extraction's line wrapping doesn't cause false negatives. Every Tier 1/2
feature above should run its output through this before showing it to the
user, the same way `rag::answer`, `extract::extract_tables`, and smart form
fill all eventually will — it's the free trust check that catches
hallucination before a value ever reaches a person.

## Architecture notes

- **Two extraction philosophies, use both:** vision-LLM direct (send page image,
  ask for fields — flexible) vs. text-first (OCR/extract, then structure —
  cheaper, loses spatial info). Production combines: specialized extractors for
  dense tables, vision models for semantic understanding, validation to catch
  errors.
- **Output format:** Markdown for RAG/retrieval (chunks well, preserves
  structure); JSON for schema-driven field extraction and automation.
- **Model selection per job:** fast/cheap for routine docs, capable for complex.
  The provider layer exposes this choice.
- **RAG stack:** Kyro (CRAG, Self-RAG, query decomposition, GraphRAG, ReAct) as
  the retrieval reference; Kohaku for episodic memory across a library; Vectro for
  embedding quantization to keep the local index small.
- **Data residency / compliance:** local by default; for cloud, document where
  data goes; audit-log what was extracted from which document (ties to Squash).

## Status

`pdfree-ai` is scaffolded: `provider.rs` defines the trait; `rag`, `ocr`,
`redact`, `extract`, `classify` expose intended signatures returning
`AiError::NotImplemented`. Bodies land in Phases 5–7. `confidence.rs` is the
one exception — fully implemented now (see above), since it needs no
provider at all.
