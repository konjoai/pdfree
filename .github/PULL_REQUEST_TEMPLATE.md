## Summary
<!-- What does this PR change and why? -->

## Type of change
- [ ] Bug fix
- [ ] New feature
- [ ] Performance improvement
- [ ] Documentation update
- [ ] Refactor / cleanup
- [ ] FFI (UniFFI) / WASM binding change

## Checklist
- [ ] `cargo test --workspace` passes locally (with PDFium fetched via `scripts/fetch-pdfium.sh`)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic` reports no errors
- [ ] `cargo fmt --all --check` passes
- [ ] No `unwrap`/`expect`/`panic!` on a production path (tests excepted)
- [ ] No hardcoded absolute paths (no `/Users/<name>/...`, no `/home/<name>/...`)
- [ ] No PDFium binaries, PDF fixtures over 500 KB, or rendered output staged for commit
- [ ] No watermark, usage limit, task cap, or hidden network upload introduced
- [ ] Changes are scoped to one logical concern (not a kitchen sink PR)
- [ ] Performance-sensitive render changes include a before/after measurement
- [ ] New form-field / annotation / page-op code paths include a test against a real PDF
- [ ] Docs (`CLAUDE.md` / `README.md` / `docs/`) updated to reflect the new state

## Related issues
<!-- Closes #123 -->
