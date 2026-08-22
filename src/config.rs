use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Jericho configuration - governs ALL resource usage, model params, RAG settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JerichoConfig {
    pub ollama: OllamaConfig,
    pub resources: ResourceLimits,
    pub model: ModelConfig,
    pub rag: RagConfig,
    pub sema: SemaConfig,
    pub gui: GuiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// Base URL for Ollama server (default: http://localhost:11434)
    pub base_url: String,
    /// Which model to use (default: qwen2.5:0.5b)
    pub model_name: String,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Number of retry attempts for failed requests
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Max RAM usage in MB (default: 2048 for 4GB system)
    pub max_ram_mb: u64,
    /// Max CPU usage percentage (0.0 - 1.0)
    pub max_cpu_percent: f32,
    /// Max number of concurrent inference threads
    pub inference_threads: u32,
    /// Enable VRAM monitoring (if GPU available)
    pub monitor_vram: bool,
    /// Max VRAM usage in MB (0 = unlimited)
    pub max_vram_mb: u64,
    /// Auto-throttle if resources exceeded
    pub auto_throttle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Temperature (0.0 - 2.0)
    pub temperature: f32,
    /// Top-p nucleus sampling
    pub top_p: f32,
    /// Top-k sampling
    pub top_k: i32,
    /// Max tokens to generate
    pub num_predict: i32,
    /// Context window size
pub num_ctx: i32,
    /// Repeat penalty
    pub repeat_penalty: f32,
    /// Enable thinking/reasoning mode (shows chain-of-thought)
    pub thinking_mode: bool,
    /// System prompt
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    /// Enable RAG pipeline
    pub enabled: bool,
    /// Chunk size for document splitting
    pub chunk_size: usize,
    /// Overlap between chunks
    pub chunk_overlap: usize,
    /// Max results to retrieve per query
    pub top_k_results: usize,
    /// Similarity threshold (0.0 - 1.0)
    pub similarity_threshold: f32,
    /// Directory to scan for documents
    pub document_dirs: Vec<String>,
    /// Supported file extensions
    pub supported_extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemaConfig {
    /// Enable offline Swahili semantic anchoring before inference
    pub enabled: bool,
    /// Path to the distilled lexicon (JSONL, CC BY-SA 4.0 - see data/NOTICE)
    pub lexicon_path: String,
    /// Lemmatize RAG tokens with Sema (fixes Swahili morphology in TF-IDF)
    pub lemmatize_rag: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiConfig {
    /// Refresh rate for system stats (ms)
    pub stats_refresh_ms: u64,
    /// Show token throughput in real-time
    pub show_token_speed: bool,
    /// Show reasoning chain (thinking tags)
    pub show_reasoning: bool,
    /// Theme: "dark", "light", "hacker"
    pub theme: String,
    /// Font scale
    pub font_scale: f32,
}

impl Default for JerichoConfig {
    fn default() -> Self {
        Self {
            ollama: OllamaConfig {
                base_url: "http://localhost:11434".to_string(),
                model_name: "qwen2.5:0.5b".to_string(),
                timeout_secs: 120,
                max_retries: 3,
            },
            resources: ResourceLimits {
                max_ram_mb: 2048,
                max_cpu_percent: 0.80,
                inference_threads: 2,
                monitor_vram: true,
                max_vram_mb: 1024,
                auto_throttle: true,
            },
            model: ModelConfig {
                temperature: 0.7,
                top_p: 0.9,
                top_k: 40,
                num_predict: 512,
                num_ctx: 2048,
                repeat_penalty: 1.1,
                thinking_mode: true,
                system_prompt: "You are Project Jericho's AI assistant. Be precise, concise, and show your reasoning when asked.".to_string(),
            },
            rag: RagConfig {
                enabled: true,
                chunk_size: 500,
                chunk_overlap: 50,
                top_k_results: 5,
                similarity_threshold: 0.3,
                document_dirs: vec!["./documents".to_string()],
                supported_extensions: vec![
                    ".txt".to_string(),
                    ".md".to_string(),
                    ".rs".to_string(),
                    ".py".to_string(),
                    ".json".to_string(),
                ],
            },
            sema: SemaConfig {
                enabled: true,
                lexicon_path: "data/swahili.distilled.jsonl".to_string(),
                lemmatize_rag: true,
            },
            gui: GuiConfig {
                stats_refresh_ms: 500,
                show_token_speed: true,
                show_reasoning: true,
                theme: "dark".to_string(),
                font_scale: 1.0,
            },
        }
    }
}

impl JerichoConfig {
    /// Config file path: ~/.config/jericho/config.toml
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("jericho").join("config.toml"))
    }

    /// Load config from file, or create default
    pub fn load() -> Self {
        match Self::config_path() {
            Some(path) => {
                if path.exists() {
                    match std::fs::read_to_string(&path) {
                        Ok(content) => match toml::from_str(&content) {
                            Ok(config) => {
                                tracing::info!("Loaded config from {}", path.display());
                                config
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse config: {}. Using defaults.", e);
                                Self::default()
                            }
                        },
                        Err(e) => {
                            tracing::warn!("Failed to read config: {}. Using defaults.", e);
                            Self::default()
                        }
                    }
                } else {
                    tracing::info!("No config found. Creating default at {}", path.display());
                    let config = Self::default();
                    config.save();
                    config
                }
            }
            None => {
                tracing::warn!("Cannot determine config directory. Using defaults.");
                Self::default()
            }
        }
    }

    /// Save current config to disk
    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match toml::to_string_pretty(self) {
                Ok(content) => {
                    if let Err(e) = std::fs::write(&path, content) {
                        tracing::error!("Failed to write config: {}", e);
                    } else {
                        tracing::info!("Config saved to {}", path.display());
                    }
                }
                Err(e) => tracing::error!("Failed to serialize config: {}", e),
            }
        }
    }
}

/// Shared config wrapped in Arc<RwLock> for thread-safe access
pub type SharedConfig = Arc<RwLock<JerichoConfig>>;
