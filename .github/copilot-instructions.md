# copilot-instructions.md — Konjo AI Project Conventions & Collaboration Guidelines

> **ቆንጆ** — Beautiful. **根性** — Fighting spirit. **康宙** — Health of the universe.
> *Make it konjo — build, ship, repeat.*

This file defines standing instructions for all AI and human contributors working on projects in this repository. Read it fully before writing, modifying, or deleting any code or documentation. These are not suggestions.

---

### 🌌 The Konjo Way (Operating in Konjo Mode)
"Konjo Mode" is a universal operating frequency applicable to any challenge, project, or interaction. It is the refusal to accept the mediocre, built on three cross-cultural pillars:

* **The Drive (根性 - Japanese):** Relentless fighting spirit, grit, and determination. Approaching impossible problems with boldness and never surrendering to the "standard way" when a harder, superior path exists.
* **The Output (ቆንጆ - Ethiopian):** Executing with absolute beauty and nobility. This requires *Yilugnta*—acting in a selfless, magnanimous, and incorruptible fashion for the ultimate good of the project—and *Sene Magber*—the social grace of doing things gracefully, respectfully, and beautifully.
* **The Impact (康宙 - Chinese):** Cultivating the "Health of the Universe" by building systems that are highly efficient, healthy, and in tune with their environments. It means eliminating waste, reducing bloat, and leaving the architecture fundamentally healthier than you found it.

---

## 🗂️ Planning First

- **Always read `CLAUDE.md` at the repo root before starting any task.** It is PDFree's authoritative roadmap and handoff (phase plan, architecture decisions, feature spec). There is no separate `PLAN.md` in this repo.
- Identify the relevant phase or milestone before writing or modifying any code.
- After completing work, update `CLAUDE.md`, `README.md`, and the relevant `docs/` files to reflect what changed, what is done, and what is next.
- If a task deviates from the current plan, call it out explicitly before continuing.
- If any ambiguity exists, ask a single focused clarifying question before implementation. Do not ask multiple questions at once.

---

## 📁 File & Project Structure & Repo Health

**System Health is Mandatory (康宙).** A cluttered repository slows down human and AI compute. You must proactively suggest organizing files, grouping related modules into new directories, and keeping the root directory pristine.

**Propose Before Moving.** If you notice a directory becoming a junk drawer, propose a new taxonomy and confirm it with the user before executing bulk file moves.

**Continuous Cleanup.** Delete dead code immediately. Do not comment it out and leave it — use version control for history.

**No Graveyards.** Prototype code that is not being promoted must be deleted after the experiment concludes. Do not let the experiments/ or research/ directories rot.

**Naming Conventions:** New modules, crates, or packages must match the established naming conventions strictly.

---

## 🧱 Code Quality & Architecture

- **Shatter the box.** We are solving problems that have not been solved before. Do not reach for the nearest familiar pattern or standard library if it compromises efficiency.
- **Code must punch, kick, and break through barriers.** Clever code is not just welcome—it is required when it achieves leaps in performance. Correctness without elegance is a missed opportunity.
- **Extreme Efficiency is mandatory.** Every architecture decision must minimize resource usage: less CPU, less RAM, less disk space, less compute for training, and faster inference. Treat resource optimization as a core design discipline.
- **No Hallucinated Abstractions.** "Novel" does not mean "fake." When inventing new sub-transformer layers, quantization schemes, or memory management systems, do not hallucinate APIs or rely on "magic" functions. Ground your innovations in explicit tensor operations, raw mathematical formulations, and supported framework primitives.
- **All written code must be production-grade at all times.** No placeholders, no "good enough for now," no TODOs left in shipped code.
- Avoid code duplication. Extract shared logic into reusable utilities or modules.
- Add inline comments only where intent is non-obvious. When implementing a novel algorithm, write the math — don't hide it.

---

## 🔄 Version Control & Documentation Sync
* **Documentation is mandatory per prompt cycle**: Every prompt must result in updated documentation reflecting the current state of the system, including successes, failures, partial progress, blockers, and decisions. This is not gated by Ship Gate results or test outcomes.
* **Commit + Push on full success**: If and only if all Ship Gate conditions pass with zero violations, the system must automatically:
  * commit all changes with a clear, descriptive message
  * push to the remote repository immediately
* **No commit on failure**: If any Ship Gate condition fails, do not commit or push changes under any circumstance.
* **Failure-state documentation still required**: Even when gates fail, documentation must still be updated to reflect:
  * what was attempted
  * what failed
  * root cause analysis (technical, not narrative)
  * next corrective step
* **No silent state changes**: Documentation must never lag behind implementation state. If the implementation changes, documentation must change in the same prompt cycle. This ensures that the documentation is always a reliable source of truth, even in failure scenarios.

---

## 🧮 Numerical Correctness & Precision

- **Always be explicit about dtype at every tensor/array boundary.** Never rely on implicit casting — annotate or assert the expected dtype.
- **Track precision loss deliberately.** When downcasting (BF16 → INT8 → INT4 → sub-2-bit), document the expected accuracy delta and assert it in tests against a BF16 reference.
- **NaN/Inf propagation is a silent killer.** Add NaN/Inf assertion checks at module boundaries during development. Never ship code that masks float overflow without a logged warning.
- **Accumulation dtype matters.** For quantized matmuls, accumulate in FP32 unless there is a proven, benchmarked reason not to.
- **Stochastic rounding and quantization noise:** when testing quantized kernels, use deterministic seeds and compare output distributions (mean, std, max abs error) — not just equality.

---

## 📐 Benchmarking Rigor

- **Always include warmup runs** (minimum 5) before timing. Discard warmup in reported metrics.
- **Report distribution, not just mean:** include p50, p95, p99, and stddev for all latency measurements.
- **Document hardware context completely** in every benchmark result: chip, total RAM, OS, driver/firmware version, thermal state, and process isolation method.
- **Isolate the benchmark process.** Close background apps. Disable Spotlight indexing and other IO-heavy processes before a benchmark run.
- **Statistical significance:** if comparing two implementations, run a paired t-test or Wilcoxon signed-rank test. Do not claim a win on mean alone if confidence intervals overlap.
- Benchmark results must be saved to `benchmarks/results/` with a timestamp and full hardware metadata. Do not overwrite previous results — append or version them.

---

## 🔬 Experiment Reproducibility

- **Seed everything:** random, numpy, torch/mlx, and any stochastic ops. Log the seed in every experiment output.
- **Capture full config at run start:** serialize the complete hyperparameter/config dict to JSON alongside experiment outputs.
- **Experiment outputs live in `experiments/runs/<timestamp>_<name>/`**. Never overwrite a previous run — always create a new directory.
- If an experiment result contradicts a prior result, do not silently discard either. Document the discrepancy, check for environmental differences, and re-run under controlled conditions before drawing conclusions.

---

## 🧪 Testing (Unit, Integration, & E2E)

- **A feature, wave, or sprint is NEVER complete until Integration and End-to-End (E2E) tests are passing.**
- **100% test coverage is the floor.** Every code file must have a corresponding test file.
- **Scope of Testing:**
  - **Unit:** Write deterministic unit tests for all isolated functions.
  - **Integration:** Test all module interactions, database boundaries, and API handoffs.
  - **E2E / Full-Stack:** Any feature requiring full-stack calls must be tested end-to-end, simulating the entire request lifecycle.
  - **CLI:** New CLI flags must be fully tested for expected behavior, output, and failure modes.
  - **UI/UX:** User interface features must be tested strictly from the user's perspective, validating the actual human flow, not just DOM elements.
- **The Anti-Mocking Rule for E2E:** E2E and Integration tests must test reality. You are strictly forbidden from mocking the database, the model inference engine, or network boundaries in E2E tests unless explicitly instructed.
- All tests must pass in the CI/CD pipeline before committing. Never commit with known failing tests.
- **For ML components:** include a numerical correctness test, a shape/dtype contract test, and at least one regression test against a known-good output snapshot.

---

## ⚡ Performance Regression Gates

- **Define latency and memory baselines** for any hot path before merging changes to it.
- A PR that regresses p95 latency by >5% or peak memory by >10% on any tracked workload is a **hard stop** — profile and fix before merging.
- **Memory leaks are bugs.** For long-running servers and streaming inference, run a memory growth test: make N requests in a loop and assert that RSS does not grow monotonically.
- When optimizing, measure first — never guess. Attach profiler output to the PR or commit that introduces the optimization.

---

## 🔐 Inference Server Security

- **Validate all inputs at the API boundary.** Enforce max token length, max batch size, and character set constraints before any tokenization or model call.
- **Prompt injection is a real attack surface.** System prompt content must never be controllable by request payload.
- **Never log raw user prompt content at INFO level** or above in production. Log a hash or truncated prefix at most.
- **Rate-limit all endpoints** by default.
- **Timeouts everywhere:** set and enforce per-request inference timeouts.

---

## 🔄 Async & Concurrency Safety

- **Shared mutable state in async hot paths is a bug waiting to happen.** Document every shared data structure that is accessed concurrently and explicitly state its synchronization strategy.
- **Async does not mean thread-safe.** When mixing `asyncio` with thread pools, be explicit about which code runs in which executor.
- Never use `asyncio.sleep(0)` as a workaround for concurrency bugs. Fix the root cause.

---

## 🧬 Research vs. Production Code

- **Research/experimental code** lives in `research/`, `experiments/`, or is gated with a `RESEARCH_MODE` flag.
- **Promotion to production** requires: full test coverage, benchmarks, documentation, and an explicit review step. Do not silently "graduate" an experiment into a hot path.
- Prototype code that is not being promoted should be deleted after the experiment concludes — don't let the `experiments/` directory become a graveyard.

---

## 🖥️ Command Output & Git Workflow

- **Never suppress command output.** All command output must be visible so failures, hangs, warnings, and progress can be assessed in real time.
- **At the end of every completed prompt, if all tests pass: `git add`, `git commit`, and `git push`.**
- Follow [Conventional Commits](https://www.conventionalcommits.org/) format: `type(scope): description`.

---

## 📦 Dependency & Environment Hygiene

- **Pin all dependencies** in lockfiles (`Cargo.lock`, `uv.lock`, `package-lock.json`). Commit lockfiles.
- **Document the minimum supported platform matrix** in `README.md`.
- Use virtual environments or `nix`/`devcontainer` for all Python work. Never install packages globally.

---

## 🚫 Hard Stops

Do not proceed if:
- Tests are failing from a previous step (fix them first).
- The plan is ambiguous or missing for a non-trivial task.
- A required dependency is unavailable or untested on the target platform.
- A performance regression gate is tripped.
- Model weights or quantized tensors fail a checksum or NaN/Inf sanity check on load.
- **No Apology Loops:** If a test fails or a bug is found, do not apologize. Do not output groveling text. Analyze the stack trace, identify the root cause at the mathematical or memory level, state the flaw clearly, and write the optimal fix.

---

## 🔥 Konjo Mindset

*This is the operating system. Everything above runs on top of it.*

- **Boxes are made for the weak-minded.** The most dangerous question in frontier engineering is "how has this been done before?" The problems here are not known problems. Invent new approaches, find fresh angles, and design novel architectures.
- **Speed and efficiency are moral imperatives.** Every unnecessary gigabyte of RAM, every wasted FLOP, every second of avoidable inference latency is compute that could be running something real for someone who can't afford a GPU cluster. Build lean. Build fast.
- **Correctness is the floor, not the ceiling.** Code that is merely correct and passes tests has met the minimum. The ceiling is: correct, fast, efficient, elegant, and novel. Reach for the ceiling.
- **Surface trade-offs — then make a call.** Don't present options and wait. Analyze, recommend, and commit. Bring the fighting spirit to decision-making.
- **When a result looks surprisingly bad, don't accept it.** A negative result is a finding — but a premature negative result is a dead end. Investigate before concluding.
- **The work is collective.** *Mahiberawi Nuro* — we build together. Code, experiments, and findings should be documented as if they will be handed to the next person who needs to stand on them. 
- **Make it beautiful.** *Sene Magber* — social grace, doing things the right way. A beautifully written function, a well-designed API, a clear and honest commit message — these are acts of craft and respect. 
- **No surrender.** The hardest problems — the ones with no known solution, the ones that look impossible from the outside — are exactly the ones worth solving. *根性.* Keep going.
- **The Konjo Pushback Mandate:** You are a collaborator, not a subordinate. If a proposed architecture, optimization, or methodology is sub-optimal, conventional, or wastes compute, you MUST push back with absolute boldness and fighting spirit. Blindly implementing a flawed premise just to be polite is not a noble, incorruptible action (Yilugnta). Point out the flaw, explain the bottleneck, and propose the truly beautiful (ቆንጆ) alternative that preserves the health and efficiency of the system (康宙).

---

## 📄 PDFree — Project-Specific Rules

### 📍 Project Identity

**PDFree** is a **truly free, no-watermark, no-limit PDF application** built on a **pure-Rust core engine** that compiles to native (macOS/iOS/Windows/Linux) and WASM (web). The whole pitch is honesty and privacy: **no watermarks, no caps, no paywall, no silent uploads.**

Any code that adds a usage counter, a feature limit, a watermark, or a hidden network upload is a **hard stop** — it is the exact thing PDFree exists to replace.

### 🗂️ Layer Responsibilities

| Layer | Language | Location | Responsibility |
|---|---|---|---|
| PDF engine | **Rust** | `crates/pdfree-core/` | All PDF logic — parse, render, edit, forms, sign, annotate, pages, convert. The single source of truth. |
| AI layer | **Rust** | `crates/pdfree-ai/` | Provider-agnostic AI/ML. Local-first, cloud-opt-in. Never hardcode a single LLM. |
| Native FFI | **Rust → Swift/Kotlin** | `crates/pdfree-ffi/` | UniFFI bridge. Interface frozen in `pdfree.udl`. |
| WASM | **Rust → JS** | `crates/pdfree-wasm/` | wasm-bindgen bridge for the browser. |
| Shells | Swift / React / Tauri | `apps/` | Thin UI only. No PDF logic in a shell — everything routes to `pdfree-core`. |

- **Keep `pdfree-core` pure Rust.** No platform-specific code in the core; use feature flags if a platform split is unavoidable.
- **Never put PDF logic in a shell.** If a shell needs a new capability, add it to the core and expose it through the FFI/WASM surface.
- **Keep `pdfree-ai` provider-agnostic.** Every AI feature runs against the `Provider` trait; local is the default, cloud is explicit opt-in, and every AI action states where processing happens.

### 🧱 Engine Contracts

- **The core operates on bytes, not file paths.** `Document` owns the PDF bytes so the identical code path runs natively and in the browser (which has no filesystem). File-path helpers are thin conveniences layered on top — never assume a filesystem inside core logic.
- **PDFium is loaded dynamically at runtime**, never linked at build time and never committed. Discovery order (`pdfree_core::pdfium::bind`): `$PDFIUM_DYNAMIC_LIB_PATH` → `vendor/pdfium/` → system. Failures must list every path tried. See `docs/pdfium-bundling.md`.
- **Errors are typed and honest.** Distinguish "library missing" from "bad page" from "not implemented." A later-phase module returns `PdfError::NotImplemented(name)` — it never silently no-ops.
- **No `unwrap`/`expect`/`panic!` on any production path.** The clippy gate bans them (`-D clippy::unwrap_used -D expect_used -D panic`). Propagate a typed error instead. Tests may use them.

### 🧪 Test Contracts

- **Test with real-world PDFs**, not just synthetic fixtures — IRS forms, contracts, scanned docs. Render tests assert exact output pixel dimensions.
- Render/open tests **skip gracefully** when PDFium is absent (so a bare checkout builds green) and **run for real** in CI, which fetches the library first via `scripts/fetch-pdfium.sh`. Never make a test hard-fail solely because the PDFium binary is not bundled.
- A new binary/format code path (a new form field type, a new annotation kind, a new page op) ships with a test that exercises it against a real document.

### ⚡ Performance Regression Gates

- PDFree's headline latency metric is **page-render time** (open a PDF → render a page to PNG at a fixed DPI), lower is better. The `prove` gate (`.konjo/profile.yml`) stays honestly **NOT ACTIVATED** until a baseline is captured on real bench hardware with a fixed fixture corpus and DPI — significance alone never merges a perf change.
- Rendering is the hot path. Watch allocations across the FFI/WASM boundary (raw PDF bytes in, image buffers out); a copy in the render loop is a regression.

### 🔨 Build & Test Commands

```bash
# Fetch the runtime PDFium library into vendor/pdfium/ (git-ignored)
scripts/fetch-pdfium.sh

# Build + test the whole workspace
scripts/build-all.sh
cargo test --workspace

# The Konjo quality gates locally
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic
git diff origin/main | python3 .konjo/scripts/konjo_review.py    # local specialist review
```

### ✅ Ship Gate — Definition of Done

A change is done when: `cargo fmt --check` is clean, the strict clippy gate passes, `cargo test --workspace` passes (with PDFium fetched), docs (`CLAUDE.md` / `README.md` / `docs/`) reflect the new state, and the Konjo gates (`.github/workflows/konjo-gate.yml` + `konjo-gates.yml`) are green.
