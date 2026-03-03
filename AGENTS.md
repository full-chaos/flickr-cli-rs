# Agent Instructions

## Project Overview

Rust CLI tool for Flickr photo management. Three crates in a workspace:

- **`flickr-cli`** — Binary. Clap-based CLI, command dispatch, user-facing config.
- **`flickr-api`** — Library. Flickr OAuth 1.0a auth + REST API client.
- **`dedupe-engine`** — Library. Image deduplication with pluggable backends (phash, ONNX, CoreML) behind Cargo features.

## Build & Test

```bash
# Check everything compiles
cargo check --workspace

# Run all tests (110 tests across 3 crates)
cargo test --workspace

# Lint (must pass with zero warnings — CI enforces this)
cargo clippy --workspace --all-targets -- -D warnings

# Format
cargo fmt --all
```

## Architecture

```
crates/
  flickr-cli/src/
    main.rs            Entrypoint — parses CLI args, dispatches to commands
    cli.rs             Clap structs (Cli, Commands, ScanBy)
    config.rs          Env var loading (FLICKR_API_KEY, FLICKR_API_SECRET)
    commands/
      auth.rs          OAuth flow — calls flickr_api::FlickrAuth
      scan.rs          Metadata-based duplicate scan (title, filename, datetaken)
      fuzzy_scan.rs    Fuzzy title matching via RapidFuzz
      sync_flickr.rs   Bulk photo download with progress bar
      ai_dedupe.rs     AI dedup — dispatches to dedupe_engine
      benchmark.rs     Benchmark all available dedup methods

  flickr-api/src/
    auth.rs            OAuth 1.0a (request token, authorize, access token, keyring storage)
    client.rs          FlickrClient — get_user_id, fetch_all_photos, download_photo
    types.rs           API types (Photo, OAuthTokens, PhotosResponse, etc.)

  dedupe-engine/src/
    engine.rs          DedupeEngine — method dispatch, image collection, DedupeMethod enum
    models.rs          Vision model configs (CLIP, SigLIP2) with HuggingFace repo IDs
    phash.rs           Perceptual hash backend (dHash via image_hasher)
    onnx.rs            ONNX Runtime backend — loads session, computes embeddings
    preprocess.rs      Image preprocessing pipeline (resize, crop, normalize, CHW tensor)
    similarity.rs      Cosine similarity, L2 normalization, pair finding
    download.rs        HuggingFace Hub model downloader
```

## Key Patterns

**Feature flags** — `dedupe-engine` uses Cargo features to gate heavy optional dependencies:
- `phash` (default) — perceptual hashing, no external model needed
- `onnx` — ONNX Runtime (`ort` crate), requires model files
- `coreml` — CoreML EP on macOS (implies `onnx`)
- `download` — auto-download models from HuggingFace
- `full` — all of the above

Code behind features uses `#[cfg(feature = "...")]` on both `mod` declarations and `match` arms.

**Workspace lints** — Defined in root `Cargo.toml` under `[workspace.lints.clippy]`. Each crate inherits via `[lints] workspace = true`. Key settings:
- `clippy::all` = deny (basic warnings are build errors)
- `clippy::pedantic` = warn (stricter checks, with targeted allows)
- `unsafe_code` = deny

**Error handling** — Library crates use `thiserror` enums (`AuthError`, `ClientError`, `DedupeError`). The binary crate uses `anyhow::Result` at the command level.

**Testing** — Tests live alongside source in `#[cfg(test)] mod tests`. `flickr-api` uses `wiremock` for HTTP mocking. Env var tests use a `Mutex` to prevent races. The keyring tests handle CI gracefully (keyring may not be available).

## CI

GitHub Actions (`.github/workflows/ci.yml`) runs on push to `main` and all PRs:

| Job | What it checks |
|-----|---------------|
| **fmt** | `cargo fmt --all --check` |
| **clippy** | `cargo clippy --workspace --all-targets -- -D warnings` |
| **test** | `cargo test --workspace` on ubuntu + macos |
| **msrv** | `cargo check --workspace` on Rust 1.75 |
| **features** | `--no-default-features` and `phash`-only compile checks |

## Adding a New Command

1. Add variant to `Commands` enum in `crates/flickr-cli/src/cli.rs`
2. Create `crates/flickr-cli/src/commands/your_command.rs` with a `pub async fn run(...)` or `pub fn run(...)`
3. Add `pub mod your_command;` to `commands/mod.rs`
4. Add match arm in `main.rs`
5. Add CLI parse tests in `cli.rs`

## Adding a New Dedupe Backend

1. Create `crates/dedupe-engine/src/your_backend.rs`
2. Add feature flag in `crates/dedupe-engine/Cargo.toml`
3. Gate the module in `lib.rs` with `#[cfg(feature = "your_feature")]`
4. Add variant to `DedupeMethod` enum in `engine.rs`
5. Add match arm in `DedupeEngine::find_duplicates` (behind `#[cfg]`)
6. Update `available_methods()` and `auto_select()`

## Environment

- **FLICKR_API_KEY** / **FLICKR_API_SECRET** — Required for any Flickr API command (auth, scan, fuzzy-scan, sync-flickr)
- Tokens stored in macOS Keychain via `keyring` crate, with file fallback at `~/.flickr_tokens`
- ONNX models cached in `cache/` or downloaded to HuggingFace Hub cache
