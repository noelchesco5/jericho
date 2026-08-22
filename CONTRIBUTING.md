# Contributing to Project Jericho

Local-first AI harness — no cloud, no telemetry, no exceptions. Contributions
should keep it that way.

## Getting Set Up

1. Install [Rust](https://rustup.rs) 1.70+
2. Install [Ollama](https://ollama.com) and pull a model:
   ```bash
   ollama pull qwen2.5:0.5b
   ```
3. Build and run:
   ```bash
   cargo run --release
   ```

## Codebase Tour

| Module | Purpose |
|---|---|
| `src/main.rs` | Entry point |
| `src/app.rs` | App state + egui wiring |
| `src/config.rs` | Runtime configuration |
| `src/ollama.rs` | Ollama HTTP client, streaming inference |
| `src/system.rs` | CPU/RAM/disk/process monitoring |
| `src/gui/` | egui views (`chat`, `health`, `config_panel`, `sidebar`) |
| `src/rag/` | Local retrieval-augmented generation pipeline |

## Ground Rules

- **No network calls** other than `localhost` (Ollama). Any PR that adds a
  cloud dependency will be rejected.
- No telemetry, analytics, or phoning home.
- Keep the default model footprint small; features must run on <1GB VRAM-class
  hardware where possible.

## Style

- Run before committing:
  ```bash
  cargo fmt --check
  cargo clippy -- -D warnings
  cargo check
  ```
- Edition 2021 idioms; avoid `unsafe` unless justified in a comment.
- GUI: follow existing egui patterns in `src/gui/` (panels + sidebars).

## Commits & PRs

- Branches: `feat/<name>`, `fix/<name>`, `docs/<name>`
- Commits: imperative mood, e.g. `Add tok/s graph to health panel`
- One logical change per PR. Include screenshots/GIFs for GUI changes.
- Describe how you tested it (Ollama version, OS, model used).

## Reporting Bugs

Include: OS, `cargo --version`, `ollama --version`, model tag, and steps to
reproduce. For crashes, paste the backtrace (`RUST_BACKTRACE=1 cargo run`).

## Areas That Need Help

- More local embedding/backends for RAG
- Cross-platform polish (Linux/macOS quirks in `system.rs`)
- Better streaming UX for long chain-of-thought output
