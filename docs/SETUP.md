# SETUP GUIDE

## Prerequisites

### Rust Toolchain
```bash
# Install Rust (if not already installed)
# Windows: Download rustup-init.exe from https://rustup.rs
# Linux/macOS:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Ollama
```bash
# Windows: Download installer from https://ollama.com
# Linux:
curl -fsSL https://ollama.com | sh
# macOS:
brew install ollama
```

### Pull the Model
```bash
# Default model (380MB, 0.5B params)
ollama pull qwen2.5:0.5b

# Higher quality 8-bit version (~650MB)
ollama pull qwen2.5:0.5b-instruct-q8_0

# Verify it works
ollama run qwen2.5:0.5b "Say hello in 5 words."
```

## Building Project Jericho

```bash
cd project-jericho

# Debug build (faster compile, slower runtime)
cargo run

# Release build (slower compile, optimized runtime)
cargo run --release

# Build without running
cargo build --release
```

The binary will be at `target/release/project-jericho.exe` (Windows) or `target/release/project-jericho` (Linux/macOS).

## First Launch

1. Ensure Ollama is running: `curl http://localhost:11434` should return "Ollama is running"
2. Launch Jericho: `cargo run --release`
3. The GUI opens with a dark theme
4. Check sidebar: OLLAMA should show "ONLINE" with green indicator
5. If offline: Start Ollama, then restart Jericho
6. Type a message in the CHAT tab and press Enter

## Using RAG

1. Place documents in `./documents/` directory (or configure other dirs in CONFIG > RAG)
2. Go to the RAG tab in the GUI
3. Click "Ingest Documents" - files are chunked and indexed
4. Use "Query Test" to search your document store
5. RAG context is automatically appended to chat messages when relevant

### Supported File Types
- `.txt` - Plain text
- `.md` - Markdown
- `.rs` - Rust source
- `.py` - Python source
- `.json` - JSON data
- Extensions are configurable in CONFIG

## Configuration

### Via GUI
Navigate to CONFIG tab. Four sub-tabs:
- **MODEL**: Model name, URL, temperature, tokens, context, system prompt
- **RESOURCES**: RAM/CPU limits, thread count, auto-throttle
- **RAG**: Enable/disable, chunk settings, directories
- **GUI**: Refresh rate, font scale, theme, display toggles

### Via File
Config auto-saves to:
- Windows: `%APPDATA%/jericho/config.toml`
- Linux: `~/.config/jericho/config.toml`
- macOS: `~/Library/Application Support/jericho/config.toml`

## Troubleshooting

### "Ollama not detected"
- Start Ollama: `ollama serve`
- Check if running: `curl http://localhost:11434`
- Verify model exists: `ollama list`

### Slow token speeds
- Use release build: `cargo run --release`
- Close other RAM-heavy apps
- Reduce context size in CONFIG > MODEL > Context Size

### GUI won't render
- Ensure GPU drivers are up to date
- Try setting `WGPU_BACKEND=gl` environment variable
- egui uses wgpu for rendering; software fallback is automatic

### RAG returns no results
- Check similarity threshold (CONFIG > RAG) - lower it to 0.1
- Ensure documents are ingested (RAG tab shows document count)
- Check file extensions are in the supported list
