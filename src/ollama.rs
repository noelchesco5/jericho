use crate::config::OllamaConfig;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

// ============================================================
// OLLAMA API CLIENT
// Handles: chat, streaming, reasoning capture, model management
// ============================================================

/// Request to generate a chat response (streaming)
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<ModelOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelOptions {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: i32,
    pub num_predict: i32,
    pub num_ctx: i32,
    pub repeat_penalty: f32,
}

/// A single streaming token/response chunk from Ollama
#[derive(Debug, Clone, Deserialize)]
pub struct StreamChunk {
    pub model: String,
    pub message: Option<StreamMessage>,
    pub done: bool,
    #[serde(default)]
    pub eval_count: Option<u64>,
    #[serde(default)]
    pub eval_duration_ns: Option<u64>,
    #[serde(default)]
    pub prompt_eval_count: Option<u64>,
    #[serde(default)]
    pub prompt_eval_duration_ns: Option<u64>,
    #[serde(default)]
    pub total_duration_ns: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamMessage {
    pub role: String,
    pub content: String,
}

/// Performance stats from a completed inference
#[derive(Debug, Clone, Default)]
pub struct InferenceStats {
    pub prompt_tokens: u64,
    pub prompt_eval_ms: f64,
    pub generated_tokens: u64,
    pub generation_ms: f64,
    pub tokens_per_second: f64,
    pub total_ms: f64,
}

/// Full response including content + extracted reasoning
#[derive(Debug, Clone)]
pub struct JerichoResponse {
    pub content: String,
    pub reasoning: String,
    pub stats: InferenceStats,
}

/// Available model info from Ollama
#[derive(Debug, Clone, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: Option<u64>,
    pub digest: Option<String>,
    pub modified_at: Option<String>,
}

/// Ollama client - handles all communication with the local inference server
#[derive(Clone)]
pub struct OllamaClient {
    http: Client,
    base_url: String,
    model: String,
}

impl OllamaClient {
    pub fn new(config: &OllamaConfig) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http,
            base_url: config.base_url.clone(),
            model: config.model_name.clone(),
        }
    }

    /// Check if Ollama server is running
    pub async fn health_check(&self) -> bool {
        match self.http.get(&self.base_url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// List all locally available models
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        let url = format!("{}/api/tags", self.base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to list models: {}", e))?;

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse model list: {}", e))?;

        let mut models = Vec::new();
        if let Some(arr) = data.get("models").and_then(|v| v.as_array()) {
            for m in arr {
                models.push(ModelInfo {
                    name: m["name"].as_str().unwrap_or("unknown").to_string(),
                    size: m["size"].as_u64(),
                    digest: m["digest"].as_str().map(|s| s.to_string()),
                    modified_at: m["modified_at"].as_str().map(|s| s.to_string()),
                });
            }
        }
        Ok(models)
    }

    /// Pull a model from Ollama registry
    pub async fn pull_model(
        &self,
        model_name: &str,
        progress_tx: mpsc::Sender<String>,
    ) -> Result<(), String> {
        let url = format!("{}/api/pull", self.base_url);
        let body = serde_json::json!({
            "name": model_name,
            "stream": true
        });

        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Pull request failed: {}", e))?;

        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        for line in text.lines() {
                            if !line.trim().is_empty() {
                                let _ = progress_tx.send(line.to_string()).await;
                            }
                        }
                    }
                }
                Err(e) => return Err(format!("Stream error during pull: {}", e)),
            }
        }
        Ok(())
    }

    /// Send a chat and stream the response back via channel
    /// Returns reasoning (thinking tags) separately from final content
    pub async fn chat_stream(
        &self,
        messages: Vec<Message>,
        options: &ModelOptions,
        content_tx: mpsc::Sender<String>,
        reasoning_tx: mpsc::Sender<String>,
        stats_tx: mpsc::Sender<InferenceStats>,
    ) -> Result<(), String> {
        let url = format!("{}/api/chat", self.base_url);
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            stream: true,
            options: Some(options.clone()),
        };

        let resp = self
            .http
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Chat request failed: {}", e))?;

        let mut stream = resp.bytes_stream();
        let mut full_content = String::new();
        let mut full_reasoning = String::new();
        let mut stats = InferenceStats::default();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        for line in text.lines() {
                            if line.trim().is_empty() {
                                continue;
                            }
                            if let Ok(parsed) = serde_json::from_str::<StreamChunk>(line) {
                                if let Some(msg) = &parsed.message {
                                    // Detect thinking/reasoning blocks
                                    let content = &msg.content;
                                    if content.contains("<think>") || content.contains("</think>") {
                                        let _ = reasoning_tx.send(content.clone()).await;
                                        full_reasoning.push_str(content);
                                    } else if !full_reasoning.is_empty()
                                        && !content.contains("</think>")
                                    {
                                        // Content after reasoning but before closing tag
                                        full_reasoning.push_str(content);
                                        let _ = reasoning_tx.send(content.clone()).await;
                                    } else {
                                        let _ = content_tx.send(content.clone()).await;
                                        full_content.push_str(content);
                                    }
                                }
                                if parsed.done {
                                    // Extract performance stats
                                    if let Some(count) = parsed.eval_count {
                                        stats.generated_tokens = count;
                                    }
                                    if let Some(dur) = parsed.eval_duration_ns {
                                        stats.generation_ms = dur as f64 / 1_000_000.0;
                                    }
                                    if let Some(count) = parsed.prompt_eval_count {
                                        stats.prompt_tokens = count;
                                    }
                                    if let Some(dur) = parsed.prompt_eval_duration_ns {
                                        stats.prompt_eval_ms = dur as f64 / 1_000_000.0;
                                    }
                                    if let Some(dur) = parsed.total_duration_ns {
                                        stats.total_ms = dur as f64 / 1_000_000.0;
                                    }
                                    // Calculate tok/s
                                    if stats.generation_ms > 0.0 {
                                        stats.tokens_per_second =
                                            stats.generated_tokens as f64
                                                / (stats.generation_ms / 1000.0);
                                    }
                                    let _ = stats_tx.send(stats.clone()).await;
                                }
                            }
                        }
                    }
                }
                Err(e) => return Err(format!("Stream error: {}", e)),
            }
        }

        Ok(())
    }

    /// Simple non-streaming chat (for quick tests)
    pub async fn chat_sync(
        &self,
        messages: Vec<Message>,
        options: &ModelOptions,
    ) -> Result<JerichoResponse, String> {
        let url = format!("{}/api/chat", self.base_url);
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            stream: false,
            options: Some(options.clone()),
        };

        let resp = self
            .http
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Chat request failed: {}", e))?;

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let content = data["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let mut reasoning = String::new();
        // Extract thinking blocks from content
        if let Some(start) = content.find("<think>") {
            if let Some(end) = content.find("</think>") {
                reasoning = content[start + 7..end].to_string();
            }
        }

        let mut stats = InferenceStats::default();
        if let Some(count) = data["eval_count"].as_u64() {
            stats.generated_tokens = count;
        }
        if let Some(dur) = data["eval_duration"].as_u64() {
            stats.generation_ms = dur as f64 / 1_000_000.0;
        }
        if let Some(count) = data["prompt_eval_count"].as_u64() {
            stats.prompt_tokens = count;
        }
        if stats.generation_ms > 0.0 {
            stats.tokens_per_second =
                stats.generated_tokens as f64 / (stats.generation_ms / 1000.0);
        }

        Ok(JerichoResponse {
            content,
            reasoning,
            stats,
        })
    }

    /// Update the model this client uses
    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }

    pub fn get_model(&self) -> &str {
        &self.model
    }
}

/// Shared client for concurrent access
pub type SharedClient = Arc<tokio::sync::RwLock<OllamaClient>>;
