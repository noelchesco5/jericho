# ARCHITECTURE

## System Design

Project Jericho is a single-binary desktop application built on the immediate-mode GUI paradigm. Every component is designed for minimal memory footprint and maximum responsiveness on 4GB RAM systems.

## Component Map

```
┌─────────────────────────────────────────────────────────┐
│                    JERICHO APP (app.rs)                  │
│                   Orchestrator Layer                     │
├──────────┬──────────┬──────────┬──────────┬─────────────┤
│  OLLAMA  │  SYSTEM  │   RAG    │  CONFIG  │    GUI      │
│  CLIENT  │ MONITOR  │ PIPELINE │  MANAGER │   PANELS    │
│(ollama.rs│(system.rs│(rag/mod  │(config.rs│(gui/*.rs)   │
│          │          │  .rs)    │          │             │
├──────────┴──────────┴──────────┴──────────┴─────────────┤
│              TOKIO ASYNC RUNTIME                         │
│         (channels, streaming, background tasks)          │
└─────────────────────────────────────────────────────────┘
```

## Data Flow

### Chat Request Flow
```
User Input (GUI) 
  → ChatPanel.take_input()
  → JerichoApp.spawn_chat()
  → tokio::spawn(async { OllamaClient.chat_stream() })
  → HTTP POST /api/chat (streaming JSON)
  → content_tx/reasoning_tx/stats_tx channels
  → GUI polls channels every frame
  → ChatPanel renders streaming text
```

### Health Monitoring Flow
```
egui frame tick
  → health_timer increments by predicted_dt
  → When timer >= refresh_ms/1000:
    → SystemMonitor.refresh()
    → sysinfo refreshes all process/system data
    → Calculate RAM/CPU/disk/process stats
    → Push snapshot to history ring buffer
    → HealthPanel.update()
    → GUI renders bars, graphs, alerts
```

### RAG Pipeline Flow
```
User clicks "Ingest Documents"
  → RagPipeline.ingest_directory()
  → Read files from configured dirs
  → chunk_document() splits into word windows
  → LocalEmbedder.embed() creates TF-IDF vectors
  → Hash projection reduces to 128 dimensions
  → VectorStore.add() stores chunks + embeddings
  → VectorStore.rebuild_index() recomputes vocabulary

User queries
  → RagPipeline.query()
  → LocalEmbedder.embed(query)
  → VectorStore.search() via cosine similarity
  → Filter by similarity_threshold
  → Return top_k results as formatted context
```

## Module Details

### OllamaClient (ollama.rs)
- HTTP client via `reqwest` with configurable timeout
- Streaming: Uses `reqwest::Response::bytes_stream()` for chunked JSON parsing
- Reasoning extraction: Detects `<think>` tags in streaming content, routes to separate channel
- Stats extraction: Parses `eval_count`, `eval_duration`, `prompt_eval_count` from final chunk
- Methods: `health_check()`, `list_models()`, `pull_model()`, `chat_stream()`, `chat_sync()`

### SystemMonitor (system.rs)
- Uses `sysinfo` crate for cross-platform process/system data
- Ring buffer of 300 HealthSnapshots (5 min at 1/sec)
- Detects Ollama process by name matching
- Jericho-specific memory tracking (sums ollama + jericho process RAM)
- Throttle alerts: Compares current usage against config limits

### RagPipeline (rag/mod.rs)
- **Chunking**: Sliding window over whitespace tokens with configurable size and overlap
- **Embedding**: TF-IDF term frequency weighting + IDF document frequency scaling + hash projection to fixed dimension
- **Vector Store**: Brute-force cosine similarity search (adequate for <100K chunks)
- **Zero external models**: No sentence-transformers, no ONNX, no GPU. Pure CPU math.
- Vocabulary built from corpus on each `rebuild_index()` call

### Config System (config.rs)
- TOML serialization via `toml` crate
- Auto-creates `~/.config/jericho/config.toml` on first run
- All settings editable live from GUI
- Changes apply immediately (client recreated, monitor limits updated)

### GUI (gui/*.rs)
- **egui 0.31** immediate-mode GUI framework
- Dark hacker theme with custom Visuals (background colors, selection colors, widget states)
- Panels: SidePanel (180px fixed sidebar) + CentralPanel (active content)
- Streaming: Polls `mpsc::Receiver` channels each frame, appends to display
- Graphs: Hand-drawn via `egui::Painter` (line segments, filled polygons, grid lines)
- No retained mode, no layout engines - direct paint commands

## Threading Model

```
Main Thread (eframe)
  ├── egui render loop (60fps)
  ├── SystemMonitor refresh (configurable, default 500ms)
  └── Channel polling (try_recv every frame)

Tokio Runtime (background)
  ├── HTTP streaming from Ollama
  └── Channel sending (content_tx, reasoning_tx, stats_tx)
```

All GUI operations happen on the main thread. Only network I/O is async. System monitoring uses synchronous sysinfo calls (fast enough at 2Hz).

## Memory Budget

| Component | Estimated RAM |
|-----------|--------------|
| Ollama server (idle) | ~50MB |
| qwen2.5:0.5b Q8_0 loaded | ~650MB |
| egui + winit window | ~40MB |
| sysinfo process cache | ~5MB |
| RAG vector store (1000 chunks) | ~2MB |
| History ring buffer (300 snapshots) | ~0.1MB |
| **Total Jericho footprint** | **~80MB (excl. Ollama)** |
| **Total with Ollama+model** | **~730MB** |

On a 4GB system, this leaves ~3.2GB for OS + other applications.
