# PROJECT JERICHO

**Local AI orchestration, RAG pipeline, and system harness with real-time GUI.**

Built in Rust. Runs entirely on your machine. Zero cloud dependencies.

## What Is This?

Project Jericho is a desktop application that:
- Connects to a local Ollama inference server for LLM chat
- Streams AI reasoning (chain-of-thought) in real-time alongside responses
- Monitors your entire system (RAM, CPU, disk, process health) in a live dashboard
- Runs a local RAG (Retrieval Augmented Generation) pipeline - no external embedding models
- Gives you full control over resource usage, model parameters, and behavior via GUI config
- Displays token throughput (tok/s) per response

## Requirements

- **OS**: Windows 10+, Linux, macOS
- **RAM**: 4GB minimum (8GB recommended)
- **Rust**: 1.70+ (install via https://rustup.rs)
- **Ollama**: Installed and running (https://ollama.com)
- **Model**: `qwen2.5:0.5b` (380MB, runs in <1GB RAM)

## Quick Start

```bash
# 1. Install Ollama
# Windows: Download from https://ollama.com
# Linux/macOS:
curl -fsSL https://ollama.com | sh

# 2. Pull the model
ollama pull qwen2.5:0.5b

# 3. Start Ollama (it auto-starts on most systems)
ollama serve

# 4. Build and run Project Jericho
cd project-jericho
cargo run --release
```

## Project Structure

```
project-jericho/
├── Cargo.toml              # Dependencies and build config
├── src/
│   ├── main.rs             # Entry point, window setup
│   ├── app.rs              # Main app state, wires everything together
│   ├── config.rs           # TOML config system (auto-creates defaults)
│   ├── ollama.rs           # Ollama HTTP client (streaming + sync)
│   ├── system.rs           # System health monitor (RAM/CPU/disk/process)
│   ├── rag/
│   │   └── mod.rs          # RAG pipeline (chunking, TF-IDF embeddings, vector store)
│   └── gui/
│       ├── mod.rs          # GUI module declarations
│       ├── chat.rs         # Chat panel with streaming + reasoning display
│       ├── health.rs       # System health dashboard with live graphs
│       ├── config_panel.rs # Configuration editor (4 tabs)
│       └── sidebar.rs      # Navigation sidebar + connection status
├── docs/
│   ├── ARCHITECTURE.md     # Deep architecture documentation
│   ├── SETUP.md            # Detailed setup guide
│   └── API.md              # Internal API reference
├── config/                 # Default config templates
└── README.md               # This file
```

## GUI Panels

### CHAT
- Type messages, get streaming responses from your local LLM
- See the AI's reasoning chain (thinking tags) in real-time
- Token throughput display (tok/s) per response
- Total token counter

### HEALTH
- Live RAM usage with bar graph and history chart
- CPU usage per-core with individual bar graphs
- CPU history graph (5-minute rolling window)
- RAM history graph
- Disk usage
- Jericho process stats (PID, memory, CPU, threads)
- Ollama process detection (running/stopped, memory, CPU)
- Auto-throttle alerts when resources exceed limits

### CONFIG
- **MODEL tab**: Model name, server URL, temperature, top-p, top-k, max tokens, context size, repeat penalty, thinking mode toggle, system prompt editor
- **RESOURCES tab**: Max RAM limit, max CPU limit, inference threads, VRAM monitoring toggle, auto-throttle toggle
- **RAG tab**: Enable/disable, chunk size, overlap, top-k results, similarity threshold, document directories, supported extensions
- **GUI tab**: Stats refresh rate, font scale, token speed display toggle, reasoning display toggle, theme selector

### RAG
- View pipeline status (documents, chunks, tokens, memory usage)
- Ingest documents from configured directories
- Query test - search your document store and see retrieved context

## Configuration

Config auto-saves to `~/.config/jericho/config.toml`. Edit via the GUI or directly.

```toml
[ollama]
base_url = "http://localhost:11434"
model_name = "qwen2.5:0.5b"
timeout_secs = 120

[resources]
max_ram_mb = 2048
max_cpu_percent = 0.80
inference_threads = 2
auto_throttle = true

[model]
temperature = 0.7
top_p = 0.9
num_predict = 512
num_ctx = 2048
thinking_mode = true

[rag]
enabled = true
chunk_size = 500
chunk_overlap = 50
top_k_results = 5
similarity_threshold = 0.3
```

## RAG System

Jericho includes a **zero-dependency RAG pipeline**:
- **Chunking**: Splits documents into overlapping word-window chunks
- **Embeddings**: TF-IDF + random projection to 128-dimensional vectors (no external model needed)
- **Vector Store**: In-memory cosine similarity search
- **Supported files**: .txt, .md, .rs, .py, .json (configurable)

Place documents in `./documents/` or configure directories in the GUI.

## Resource Budget (4GB RAM)

| Component | RAM Usage |
|-----------|-----------|
| Ollama + qwen2.5:0.5b (Q8_0) | ~700MB |
| Project Jericho GUI | ~50MB |
| OS overhead | ~1GB |
| **Free for other apps** | **~2.2GB** |

## License

MIT
