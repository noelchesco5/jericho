# INTERNAL API REFERENCE

## OllamaClient

```rust
pub struct OllamaClient { ... }

impl OllamaClient {
    pub fn new(config: &OllamaConfig) -> Self;
    pub async fn health_check(&self) -> bool;
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, String>;
    pub async fn pull_model(&self, model_name: &str, progress_tx: mpsc::Sender<String>) -> Result<(), String>;
    pub async fn chat_stream(&self, messages: Vec<Message>, options: &ModelOptions, content_tx: mpsc::Sender<String>, reasoning_tx: mpsc::Sender<String>, stats_tx: mpsc::Sender<InferenceStats>) -> Result<(), String>;
    pub async fn chat_sync(&self, messages: Vec<Message>, options: &ModelOptions) -> Result<JerichoResponse, String>;
    pub fn set_model(&mut self, model: String);
    pub fn get_model(&self) -> &str;
}
```

### Key Types
- `Message { role: String, content: String }` - Chat message (system/user/assistant)
- `ModelOptions { temperature, top_p, top_k, num_predict, num_ctx, repeat_penalty }` - Inference parameters
- `InferenceStats { prompt_tokens, prompt_eval_ms, generated_tokens, generation_ms, tokens_per_second, total_ms }` - Performance metrics
- `JerichoResponse { content, reasoning, stats }` - Complete response with separated reasoning
- `StreamChunk` - Parsed JSON chunk from Ollama streaming API

---

## SystemMonitor

```rust
pub struct SystemMonitor { ... }

impl SystemMonitor {
    pub fn new(ram_limit_mb: u64, cpu_limit_percent: f32) -> Self;
    pub fn refresh(&mut self) -> SystemHealth;
    pub fn check_throttle(&self) -> Vec<ThrottleAlert>;
    pub fn get_history(&self) -> &[HealthSnapshot];
    pub fn get_last_health(&self) -> &SystemHealth;
    pub fn update_limits(&mut self, ram_mb: u64, cpu_percent: f32);
}
```

### Key Types
- `SystemHealth { timestamp, ram, cpu, disk, process, gpu, ollama_process }` - Full system snapshot
- `RamStats { total_mb, used_mb, free_mb, usage_percent, available_mb, jericho_used_mb }` - RAM details
- `CpuStats { usage_percent, core_count, brand, frequency_mhz, per_core_usage }` - CPU details
- `HealthSnapshot { timestamp, ram_percent, cpu_percent, jericho_mb, gpu_percent }` - Historical data point
- `ThrottleAlert { severity, resource, message, current, limit }` - Resource limit warning

---

## RagPipeline

```rust
pub struct RagPipeline { ... }

impl RagPipeline {
    pub fn new(config: RagConfig) -> Self;
    pub fn ingest_file(&mut self, path: &Path) -> Result<IngestedDocument, String>;
    pub fn ingest_directory(&mut self, dir: &Path, extensions: &[String]) -> Result<Vec<IngestedDocument>, String>;
    pub fn query(&self, question: &str) -> (String, Vec<RagResult>);
    pub fn stats(&self) -> RagStats;
}
```

### Supporting Types
- `DocumentChunk { id, source_file, chunk_index, content, token_estimate, metadata }` - Text chunk
- `RagResult { chunk, similarity, rank }` - Search result with cosine similarity score
- `RagStats { total_documents, total_chunks, total_tokens, last_ingest_time_ms, memory_usage_mb }` - Pipeline stats
- `IngestedDocument { path, chunks_count, total_tokens, ingested_at, file_hash }` - Ingested file record

### LocalEmbedder
```rust
impl LocalEmbedder {
    pub fn new(dim: usize) -> Self;          // Create with target dimension
    pub fn fit(&mut self, corpus: &[String]); // Build vocabulary from corpus
    pub fn embed(&self, text: &str) -> Vec<f32>; // Embed text to fixed-dim vector
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32; // Similarity metric
}
```

---

## JerichoConfig

```rust
pub struct JerichoConfig {
    pub ollama: OllamaConfig,     // Server connection
    pub resources: ResourceLimits, // Hardware budget
    pub model: ModelConfig,       // LLM parameters
    pub rag: RagConfig,          // RAG pipeline settings
    pub gui: GuiConfig,          // Display preferences
}

impl JerichoConfig {
    pub fn config_path() -> Option<PathBuf>;
    pub fn load() -> Self;       // Load from TOML or create defaults
    pub fn save(&self);          // Persist to TOML
}
```

---

## GUI Panels

### ChatPanel
```rust
impl ChatPanel {
    pub fn new() -> Self;
    pub fn render(&mut self, ui: &mut egui::Ui);
    pub fn take_input(&mut self) -> Option<String>;
    pub fn add_message(&mut self, role, content, reasoning, tps, tokens);
    pub fn start_streaming(&mut self);
    pub fn finish_streaming(&mut self);
}
```

### HealthPanel
```rust
impl HealthPanel {
    pub fn new() -> Self;
    pub fn update(&mut self, health: SystemHealth, history: Vec<HealthSnapshot>);
    pub fn render(&mut self, ui: &mut egui::Ui);
}
```

### ConfigPanel
```rust
impl ConfigPanel {
    pub fn new(config: JerichoConfig) -> Self;
    pub fn render(&mut self, ui: &mut egui::Ui);
    // Changes auto-track via dirty flag
}
```

### Sidebar
```rust
impl Sidebar {
    pub fn new() -> Self;
    pub fn render(&mut self, ui: &mut egui::Ui);
    // Returns active panel via self.active
}
```

---

## Ollama HTTP API (External)

Jericho communicates with Ollama via these endpoints:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/` | GET | Health check |
| `/api/tags` | GET | List available models |
| `/api/pull` | POST | Download a model |
| `/api/chat` | POST | Chat completion (streaming) |

### Chat Request Body
```json
{
  "model": "qwen2.5:0.5b",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."}
  ],
  "stream": true,
  "options": {
    "temperature": 0.7,
    "top_p": 0.9,
    "top_k": 40,
    "num_predict": 512,
    "num_ctx": 2048,
    "repeat_penalty": 1.1
  }
}
```

### Streaming Response Chunk
```json
{
  "model": "qwen2.5:0.5b",
  "message": {"role": "assistant", "content": "token text"},
  "done": false
}
```

### Final Chunk (done: true)
```json
{
  "model": "qwen2.5:0.5b",
  "done": true,
  "eval_count": 128,
  "eval_duration": 4200000000,
  "prompt_eval_count": 45,
  "prompt_eval_duration": 800000000,
  "total_duration": 5100000000
}
```
