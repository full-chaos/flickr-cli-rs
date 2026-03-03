# Claude Code Instructions

Read [AGENTS.md](./AGENTS.md) for project architecture, build commands, key patterns, and how-to guides.

## Quick Reference

```bash
cargo test --workspace                                    # Run all tests
cargo clippy --workspace --all-targets -- -D warnings     # Lint (must be clean)
cargo fmt --all                                           # Format
```

## Rules

- All clippy warnings are CI-enforced errors. Run clippy before committing.
- No `unsafe` code. The workspace denies it.
- Use `thiserror` for library error types, `anyhow` for binary command handlers.
- Gate optional heavy dependencies behind Cargo features (see `dedupe-engine`).
- Tests go in `#[cfg(test)] mod tests` blocks alongside the source, not in a separate `tests/` directory.
