# flickr-cli

A Rust CLI tool for managing Flickr photo libraries — duplicate detection (metadata, fuzzy, and AI-based), bulk sync, and OAuth authentication.

## Quick Start

```bash
# Build
cargo build --release

# Set up Flickr API credentials (https://www.flickr.com/services/apps/create/)
export FLICKR_API_KEY="your_key"
export FLICKR_API_SECRET="your_secret"

# Authenticate (opens browser for OAuth 1.0a flow)
flickr-cli auth

# Scan for duplicate photos by title/filename
flickr-cli scan

# AI-based duplicate detection on local images
flickr-cli ai-dedupe --directory ~/Photos
```

## Commands

### `auth`

Authenticate with Flickr via OAuth 1.0a. Opens a browser for authorization, then stores tokens in the system keychain (macOS Keychain, etc.) with file fallback at `~/.flickr_tokens`.

```bash
flickr-cli auth
```

### `scan`

Scan your Flickr library for duplicates by metadata fields.

```bash
# Default: group by title + filename
flickr-cli scan

# Custom fields
flickr-cli scan --by title,datetaken
```

Fields: `title`, `filename`, `datetaken`

### `fuzzy-scan`

Find near-duplicate photo titles using fuzzy string matching (RapidFuzz).

```bash
# Default threshold: 85%
flickr-cli fuzzy-scan

# Stricter matching
flickr-cli fuzzy-scan --threshold 95
```

### `sync-flickr`

Download all Flickr photos to a local directory. Skips already-downloaded files.

```bash
flickr-cli sync-flickr --directory ~/flickr-backup

# Limit to first 500 photos
flickr-cli sync-flickr --directory ~/flickr-backup --max-images 500
```

### `ai-dedupe`

AI-based duplicate detection on local images using perceptual hashing or vision model embeddings.

```bash
# Auto-select best available method
flickr-cli ai-dedupe --directory ~/Photos

# Specific method + model
flickr-cli ai-dedupe --directory ~/Photos --method onnx --model siglip2-b16

# Lower threshold to catch more near-duplicates
flickr-cli ai-dedupe --directory ~/Photos --similarity-threshold 0.85 --max-images 1000
```

**Methods:**

| Method | Feature | Description |
|--------|---------|-------------|
| `phash` | `phash` (default) | Perceptual hashing (dHash) — fast, no model needed |
| `onnx` | `onnx` | Vision model embeddings via ONNX Runtime |
| `coreml` | `coreml` | ONNX with CoreML acceleration (macOS only) |

**Vision models** (for `onnx`/`coreml`):

| Model | Input | Embedding | Notes |
|-------|-------|-----------|-------|
| `clip-vit-b32` | 224px | 512-dim | Original CLIP, fastest |
| `siglip2-b16` | 224px | 768-dim | **Default.** Best quality/speed ratio |
| `siglip2-so400m` | 384px | 1152-dim | Highest quality, ~7GB RAM |

### `benchmark-methods`

Compare deduplication methods on a set of images.

```bash
flickr-cli benchmark-methods --directory ~/Photos --num-images 20
```

## Building with Features

The `dedupe-engine` crate uses Cargo features to control which backends are compiled:

```bash
# Default (phash only — no external dependencies)
cargo build --release

# With ONNX Runtime support
cargo build --release --features dedupe-engine/onnx

# With auto-download from HuggingFace
cargo build --release --features dedupe-engine/download

# Everything
cargo build --release --features dedupe-engine/full
```

## Project Structure

```
crates/
  flickr-cli/       Binary — CLI interface (clap), command dispatch
  flickr-api/       Library — Flickr OAuth 1.0a + REST client
  dedupe-engine/    Library — Image dedup (phash, ONNX, CoreML)
```

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `FLICKR_API_KEY` | Yes | Flickr API key ([create one](https://www.flickr.com/services/apps/create/)) |
| `FLICKR_API_SECRET` | Yes | Flickr API secret |

## Requirements

- Rust 1.75+
- Flickr API credentials
- macOS recommended (for Keychain token storage and CoreML support)

## License

MIT
