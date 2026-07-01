# Handoff: `konjo-gates` repo-native checks fail instantly on the `lang/rust` pack

**For:** whoever picks this up next with `konjoai/kiban` access (Wes, or a
fresh Claude Code session scoped to that repo — this session's access is
`konjoai/pdfree` only, so the investigation stopped at the boundary of what's
visible from the consuming side).

**tl;dr:** In `konjoai/pdfree`, `konjo-gates` (pinned `KIBAN_REF: v1.1.0`)
reports all four `lang/rust` pack repo-native gates
(`repo:fmt-check`, `repo:clippy`, `repo:cargo-deny`, `repo:cargo-mutants`) as
failed with a bare `net-new findings` message and no further detail — and it
does so in about **0.2 seconds total**, which is not physically enough time
for those tools to have actually run (`cargo-mutants` alone needs minutes
just for its baseline `cargo test` pass). Something in the `lang/rust` pack's
gate-invocation path is failing before or instead of actually shelling out to
the tools, and reporting that failure as generic "findings" instead of a
distinguishable tool error.

## Reproducing

```bash
git clone https://github.com/konjoai/pdfree.git
cd pdfree
git checkout claude/pdfree-architecture-nkhtgs   # or main, once merged — the profile is unchanged
scripts/fetch-pdfium.sh
pip install "kiban @ git+https://github.com/konjoai/kiban.git@v1.1.0"
konjo-gates --profile .konjo/profile.yml --base "origin/main"
```

Expected (based on the module docstrings and what Wall-2's own separately-run
`cargo fmt`/`clippy`/`cargo-deny` steps report): `repo:fmt-check` and
`repo:clippy` pass cleanly (confirmed locally — see below); `repo:cargo-deny`
passes once `.konjo/deny.toml` is on the modern schema (see "Already ruled
out" below); `repo:cargo-mutants` takes real wall-clock time and reports
actual mutation survivors or a clean pass.

Actual, on the last CI run (`konjoai/pdfree` PR #4, run
[28524169051](https://github.com/konjoai/pdfree/actions/runs/28524169051)):

```
konjo-gates: 20 changed file(s), base origin/main
  [WARN ] prose: net-new in general docs: docs/api.md (em-dash:'—')
  [PASS ] secrets: no net-new secrets
  [PASS ] one_way_door: two-way door
  [SKIP ] prove: not a perf change
  [SKIP ] longrun: no changed long-run scripts
  [PASS ] self_test: 5 must-flag, 1 control(s), runs=3
  [PASS ] verify_cmd: declared: cargo nextest run --workspace
  [PASS ] context_budget: always-on ~0 tok <= 1500 ceiling
  [PASS ] skill_size: all skills within 80 lines (or justified)
  [PASS ] specialist_stats: no review history yet
  [FAIL ] repo:fmt-check: net-new findings
  [FAIL ] repo:clippy: net-new findings
  [FAIL ] repo:cargo-deny: net-new findings
  [PASS ] repo:unsafe-budget: no net-new unjustified unsafe (added unjustified 0, removed 0)
  [FAIL ] repo:cargo-mutants: net-new findings
konjo-gates: BLOCKED (4 gate(s) failed)
```

Every kiban-native gate (`prose`, `secrets`, `one_way_door`, `self_test`,
`verify_cmd`, `context_budget`, `skill_size`, `specialist_stats`,
`unsafe-budget`) passes or reports something specific. Only the four
`repo:*` gates that are supposed to shell out to real cargo subcommands fail,
and they fail identically and instantly every time.

## The timing evidence

From the raw GitHub Actions log timestamps (job
[84555976274](https://github.com/konjoai/pdfree/actions/runs/28524168836/job/84555976274)):

```
14:19:19.7156Z  ##[group]Run konjo-gates --profile .konjo/profile.yml --base "origin/main"
...
14:19:19.9167Z  [WARN ] prose: ...
14:19:19.9172Z  konjo-gates: BLOCKED (4 gate(s) failed)
```

**~0.2 seconds** from invocation to the full report, all 19 gates included.
This was on a CI job that — after a fix in this repo (see below) —
had already spent ~3 minutes compiling `cargo-deny` and `cargo-mutants` from
source immediately beforehand, so both tools were confirmed present and on
`PATH` when `konjo-gates` ran. `cargo-mutants` requires a baseline
`cargo test`/`cargo nextest` build+run before it can mutate anything at all;
that alone takes well over 0.2s on any real Rust workspace, let alone a full
mutation sweep. This strongly suggests `konjo_gates_py`'s `repo:*` gate
handlers for the `lang/rust` pack are not actually invoking these tools —
they're failing (or short-circuiting) somewhere upstream of the subprocess
call, and that failure is being reported as "findings" rather than surfaced
as a distinguishable error (missing tool, config parse error, exception,
etc).

## What's already been ruled out (fixed in `pdfree`, didn't help)

Two real, unrelated bugs were found and fixed in `pdfree` while chasing this
— neither one resolved the `repo:*` gate failures, but both were necessary
prerequisites and are worth knowing about so they aren't re-diagnosed:

1. **`5f7c39b` — `.gitattributes` missing `*.pdf binary`.** Before this,
   `konjo-gates` crashed outright with `UnicodeDecodeError` while decoding
   `git diff` output, because a hand-built PDF test fixture had no NUL byte
   in its first ~8KB and so wasn't classified as binary by git — raw PDF
   bytes leaked into the diff text `konjo_gates_py/cli.py`'s `_diff_text()`
   decodes as UTF-8. Fixed by adding `.gitattributes`. This is a real,
   independent robustness gap in `konjo_gates_py`: it should not crash on a
   diff containing non-UTF-8 bytes regardless of what the consuming repo's
   `.gitattributes` does or doesn't declare — worth a defensive fix
   (`errors="replace"` or similar) in `_diff_text()`/`_git()` in
   `konjo_gates_py/cli.py` regardless of the main issue below.

2. **`b377b96` — `.konjo/deny.toml` schema drift.** The file (copied
   verbatim from `vectro`) used a `cargo-deny` config schema that's since
   been removed upstream: `[licenses]` no longer has `copyleft`/`unlicensed`
   keys (licenses are allow-list-only now), and `[advisories]` no longer has
   `vulnerability`/`unmaintained`/`notice` keys (vulnerabilities/yanked
   crates are denied by default now, with no per-category severity dial).
   Confirmed locally: `cargo-deny` 0.19.9 refuses to parse the old schema at
   all (`unexpected-value` on `unmaintained = "warn"`). Fixed by rewriting to
   the modern schema. **This may also affect `vectro`'s own `.konjo/deny.toml`**
   if it hasn't been updated since — worth checking, since `pdfree`'s file
   was a direct copy.

3. **`b377b96` — missing Rust toolchain in `konjo-gates.yml`.** The thin
   per-repo workflow template (`templates/repo-ci.yml` in kiban) only sets up
   Python and pip-installs kiban — it never installs a Rust toolchain or the
   third-party `cargo-deny`/`cargo-mutants` subcommands. `rustfmt`/`clippy`
   might come from whatever Rust ships on the `ubuntu-latest` GitHub-hosted
   runner image by default, but `cargo-deny` and `cargo-mutants` definitely
   don't. Fixed on the `pdfree` side by adding a toolchain-setup step to
   `.github/workflows/konjo-gates.yml`. **If the `lang/rust` pack's `repo:*`
   gates are meant to be usable out of the box via the `templates/repo-ci.yml`
   template, that template is currently incomplete** — it should either
   install these tools itself, or `konjo_gates_py` should detect a missing
   tool and report that distinctly (`SKIP` or a clear "tool not found"
   message) rather than folding it into "findings".

None of these three fixes changed the `repo:*` gate outcome or timing at all
— same four failures, same ~0.2s, before and after.

## What to check in `kiban`'s source

Start from the traceback captured during bug #1 above, which is the only
concrete pointer into `konjo_gates_py`'s internals visible from the consuming
side:

```
File ".../site-packages/konjo_gates_py/cli.py", line 581, in main
    diff_text = _diff_text(args.base)
File ".../site-packages/konjo_gates_py/cli.py", line 143, in _diff_text
    return _git(["diff", f"{base}...HEAD"]) + _git(["diff", "HEAD"])
File ".../site-packages/konjo_gates_py/cli.py", line 119, in _git
    proc = subprocess.run(["git", *args], capture_output=True, text=True)
```

From there:

1. **Find where `repo:fmt-check`/`repo:clippy`/`repo:cargo-deny`/
   `repo:cargo-mutants` are dispatched** — almost certainly in
   `lib/packs/lang/rust` (per kiban's own `CHANGELOG.md`/`README.md`
   phase-7 entry: *"a new `rust` pack (`ownership-lifetimes`,
   `error-handling`, `perf-alloc`, the `unsafe-budget` gate, and the cargo
   tool table)"* — "the cargo tool table" is the thing to find).
2. **Check how each tool is invoked** — `subprocess.run`, likely with
   `shutil.which` or a bare command name. If a tool isn't found, does it
   raise, return an empty/error result that gets silently coerced into a
   generic "findings" failure, or something else? Given `repo:unsafe-budget`
   (also part of the same `lang/rust` pack, also a "cargo tool table" entry
   per the changelog) reports a specific, correct-looking result
   (`no net-new unjustified unsafe`) while the other four don't, the bug is
   likely specific to how `fmt-check`/`clippy`/`cargo-deny`/`cargo-mutants`
   are each invoked or how their output is parsed — not a pack-wide issue.
3. **Check for a net-new diffing step that runs *before* the tool even
   executes** — the summary format is literally `net-new findings`, which
   matches `bin/konjo-newonly`'s wrapper pattern (mentioned in kiban's
   `README.md`: *"Strict gates report only net-new findings versus the base
   ref"*). If `konjo-newonly` itself is failing (e.g. trying to run the tool
   against a worktree of the base ref, in an environment where a second
   checkout/build isn't set up correctly in this pack), that would produce
   exactly this symptom — instant, generic failure, no detail, and it would
   explain why it's *consistent* across all four gates that go through that
   wrapper.
4. **Try reproducing outside CI**, with a real local Rust checkout and both
   `--base` pointing somewhere valid and an actual clean run of each tool
   manually, to see whether `konjo-newonly`'s wrapping is where it breaks
   vs. the tool-specific handler itself.
5. **Check for silent exception handling** — a `try/except: return
   Verdict(passed=False, findings=["net-new findings"])`-shaped catch-all
   somewhere in the `repo:*` gate dispatch would produce exactly this
   symptom (uniform message, sub-second timing, real bugs masked as
   findings). If one exists, at minimum it should log or surface the actual
   exception rather than swallowing it into an indistinguishable "findings"
   message — that alone would have made this a five-minute diagnosis instead
   of a multi-hour one from the consuming side.

## Current state / what's blocking on this

`pdfree`'s `.github/workflows/konjo-gates.yml` has `continue-on-error: true`
on the `gates` job (commit `ad93031`) as a deliberate, temporary workaround
— it's not blocking merges right now. `pdfree`'s actual quality enforcement
is coming from `build + test` (the plain `cargo` CI) and the separate
Wall-2 `konjo-gate.yml` workflow (which runs `cargo fmt`/`clippy`/`cargo
audit`/`cargo deny`/`cargo mutants` directly, no kiban involved, and is
green). So there's no urgency — but the `lang/rust` pack's repo-native gates
are currently non-functional for any repo using them, which presumably
affects `vectro` too if it's pinned to `v1.1.0` or later.

Once a fix lands in `kiban`, re-enable enforcement in `pdfree` by removing
`continue-on-error: true` from the `gates` job in
`.github/workflows/konjo-gates.yml` and the accompanying `TODO(pdfree)`
comments there and in `.konjo/profile.yml`, then bump `KIBAN_REF` (currently
pinned to `v1.1.0`) once a fixed version is tagged.
