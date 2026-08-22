use crate::config::{JerichoConfig, SharedConfig};
use crate::gui::chat::{ChatPanel, MessageRole};
use crate::gui::config_panel::ConfigPanel;
use crate::gui::health::HealthPanel;
use crate::gui::sidebar::{Sidebar, ActivePanel};
use crate::ollama::{self, OllamaClient, Message, ModelOptions, SharedClient};
use crate::rag::{self, RagPipeline, RagConfig};
use crate::sema_anchor::{self, Anchor};
use crate::system::{SystemMonitor, SharedMonitor};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc;

/// Maximum conversation turns sent back to the model as context
const MAX_HISTORY_MESSAGES: usize = 20;

pub struct JerichoApp {
    config: JerichoConfig,
    client: OllamaClient,
    monitor: SystemMonitor,
    rag: Option<RagPipeline>,
    /// Offline Swahili semantic anchoring (Sema), when enabled + lexicon found
    anchor: Option<Anchor>,
    sidebar: Sidebar,
    chat_panel: ChatPanel,
    health_panel: HealthPanel,
    config_panel: ConfigPanel,
    runtime: tokio::runtime::Runtime,
    content_rx: Option<mpsc::Receiver<String>>,
    reasoning_rx: Option<mpsc::Receiver<String>>,
    stats_rx: Option<mpsc::Receiver<ollama::InferenceStats>>,
    error_rx: Option<mpsc::Receiver<String>>,
    active_panel: ActivePanel,
    ollama_connected: bool,
    initialized: bool,
    pending_send: bool,
    health_timer: f64,
    /// Throttle alerts: cooldown per resource type (30s)
    alert_cooldowns: std::collections::HashMap<String, std::time::Instant>,
    /// Models reported by the Ollama server
    available_models: Vec<String>,
    /// Model chosen for this session (hot-swappable from chat header)
    selected_model: String,
}

impl JerichoApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(14, 16, 20);
        visuals.window_fill = egui::Color32::from_rgb(18, 20, 26);
        visuals.extreme_bg_color = egui::Color32::from_rgb(10, 12, 16);
        visuals.faint_bg_color = egui::Color32::from_rgb(22, 26, 32);
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(22, 26, 32);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(26, 30, 38);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(30, 35, 45);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(35, 40, 55);
        visuals.selection.bg_fill = egui::Color32::from_rgb(40, 80, 60);
        visuals.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(100, 255, 140));
        cc.egui_ctx.set_visuals(visuals);

        let config = JerichoConfig::load();
        let client = OllamaClient::new(&config.ollama);
        let monitor = SystemMonitor::new(
            config.resources.max_ram_mb,
            config.resources.max_cpu_percent,
        );
        let mut rag = if config.rag.enabled {
            Some(RagPipeline::new(RagConfig {
                chunk_size: config.rag.chunk_size,
                chunk_overlap: config.rag.chunk_overlap,
                top_k: config.rag.top_k_results,
                similarity_threshold: config.rag.similarity_threshold,
                embedding_dim: 128,
            }))
        } else {
            None
        };
        let anchor = if config.sema.enabled {
            match sema_anchor::load_anchor(&config.sema.lexicon_path) {
                Ok(a) => {
                    tracing::info!("Sema lexicon loaded: {} lemmas", a.lemma_count());
                    Some(a)
                }
                Err(e) => {
                    tracing::warn!("Sema enabled but lexicon unavailable: {}", e);
                    None
                }
            }
        } else {
            None
        };
        if let (Some(anchor), Some(rag)) = (&anchor, rag.as_mut()) {
            if config.sema.lemmatize_rag {
                rag.set_lemmatizer(Some(anchor.lexicon()));
            }
        }
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        let selected_model = config.ollama.model_name.clone();

        Self {
            config: config.clone(),
            client,
            monitor,
            rag,
            anchor,
            sidebar: Sidebar::new(),
            chat_panel: ChatPanel::new(),
            health_panel: HealthPanel::new(),
            config_panel: ConfigPanel::new(config),
            runtime,
            content_rx: None,
            reasoning_rx: None,
            stats_rx: None,
            error_rx: None,
            active_panel: ActivePanel::Chat,
            ollama_connected: false,
            initialized: false,
            pending_send: false,
            health_timer: 0.0,
            alert_cooldowns: std::collections::HashMap::new(),
            available_models: Vec::new(),
            selected_model,
        }
    }

    pub fn initialize(&mut self) -> Result<(), String> {
        tracing::info!("Project Jericho GUI initialized");

        let client = self.client.clone();
        self.ollama_connected = self.runtime.block_on(client.health_check());
        self.sidebar.ollama_connected = self.ollama_connected;
        self.sidebar.model_name = self.config.ollama.model_name.clone();

        if self.ollama_connected {
            tracing::info!("Ollama server detected at {}", self.config.ollama.base_url);
            self.chat_panel.add_message(
                MessageRole::System,
                format!("Connected to Ollama at {}", self.config.ollama.base_url),
                String::new(),
                0.0,
                0,
            );
            let client2 = self.client.clone();
            match self.runtime.block_on(client2.list_models()) {
                Ok(models) => {
                    let names: Vec<String> = models.iter().map(|m| m.name.clone()).collect();
                    self.available_models = names.clone();
                    // Fall back to the first installed model if the configured
                    // one is not present on this machine.
                    if !names.contains(&self.selected_model) {
                        if let Some(first) = names.first() {
                            tracing::warn!(
                                "Configured model '{}' not installed; using '{}' instead",
                                self.selected_model,
                                first
                            );
                            self.selected_model = first.clone();
                            self.config.ollama.model_name = first.clone();
                        }
                    }
                    if names.is_empty() {
                        self.chat_panel.add_message(
                            MessageRole::System,
                            "No models installed yet. Run: ollama pull qwen2.5:0.5b".to_string(),
                            String::new(),
                            0.0,
                            0,
                        );
                    } else {
                        self.chat_panel.add_message(
                            MessageRole::System,
                            format!("Available models ({}): {}", names.len(), names.join(", ")),
                            String::new(),
                            0.0,
                            0,
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("Could not list models: {}", e);
                }
            }
        } else {
            self.chat_panel.add_message(
                MessageRole::System,
                "WARNING: Ollama not detected. Start Ollama and run: ollama pull qwen2.5:0.5b".to_string(),
                String::new(),
                0.0,
                0,
            );
        }

        self.sidebar.model_name = self.selected_model.clone();

        match &self.anchor {
            Some(a) => {
                let rag_note = if self.config.sema.lemmatize_rag && self.rag.is_some() {
                    " + RAG lemmatization"
                } else {
                    ""
                };
                self.chat_panel.add_message(
                    MessageRole::System,
                    format!(
                        "Sema anchoring active: {} Swahili lemmas{}",
                        a.lemma_count(),
                        rag_note
                    ),
                    String::new(),
                    0.0,
                    0,
                );
            }
            None if self.config.sema.enabled => {
                self.chat_panel.add_message(
                    MessageRole::System,
                    "Sema enabled but lexicon not found - expected at data/swahili.distilled.jsonl".to_string(),
                    String::new(),
                    0.0,
                    0,
                );
            }
            _ => {}
        }

        let health = self.monitor.refresh();
        self.health_panel.update(health, self.monitor.get_history().to_vec());

        self.initialized = true;
        Ok(())
    }

    fn spawn_chat(&mut self, input: String) {
        let (content_tx, content_rx) = mpsc::channel(256);
        let (reasoning_tx, reasoning_rx) = mpsc::channel(256);
        let (stats_tx, stats_rx) = mpsc::channel(32);
        let (error_tx, error_rx) = mpsc::channel(8);

        self.content_rx = Some(content_rx);
        self.reasoning_rx = Some(reasoning_rx);
        self.stats_rx = Some(stats_rx);
        self.error_rx = Some(error_rx);
        self.pending_send = true;
        self.chat_panel.start_streaming();

        // Build conversation history so the model remembers prior turns.
        // The current input was just pushed to the panel; it is re-added
        // explicitly below, so drop the trailing copy here.
        let mut history: Vec<Message> = self
            .chat_panel
            .messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::User | MessageRole::Assistant))
            .map(|m| Message {
                role: if m.role == MessageRole::User {
                    "user".to_string()
                } else {
                    "assistant".to_string()
                },
                content: m.content.clone(),
            })
            .collect();
        history.pop();
        if history.len() > MAX_HISTORY_MESSAGES {
            let start = history.len() - MAX_HISTORY_MESSAGES;
            history.drain(..start);
        }

        let client = self.client.clone();
        let model = self.selected_model.clone();
        self.chat_panel.active_model = model.clone();
        let system_prompt = self.config.model.system_prompt.clone();
        let options = ModelOptions {
            temperature: self.config.model.temperature,
            top_p: self.config.model.top_p,
            top_k: self.config.model.top_k,
            num_predict: self.config.model.num_predict,
            num_ctx: self.config.model.num_ctx,
            repeat_penalty: self.config.model.repeat_penalty,
        };

        // Anchor pass (Sema): prepend English glosses to user message so
        // the model has word-level context. English passes through untouched.
        let user_content = match &self.anchor {
            Some(anchor) => {
                let block = anchor.prompt_block(&input);
                if block.is_empty() {
                    input.clone()
                } else {
                    format!("{block}\n---\n{input}")
                }
            }
            None => input.clone(),
        };

        self.runtime.spawn(async move {
            let mut messages = Vec::with_capacity(history.len() + 2);
            messages.push(Message {
                role: "system".to_string(),
                content: system_prompt,
            });
            messages.extend(history);
            messages.push(Message {
                role: "user".to_string(),
                content: user_content,
            });

            if let Err(e) = client
                .chat_stream(&model, messages, &options, content_tx, reasoning_tx, stats_tx)
                .await
            {
                tracing::error!("Chat stream error: {}", e);
                let _ = error_tx.send(e).await;
            }
        });
    }
}

impl eframe::App for JerichoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Periodic health refresh + harness throttle alerts
        self.health_timer += ctx.input(|i| i.predicted_dt) as f64;
        if self.health_timer >= self.config.gui.stats_refresh_ms as f64 / 1000.0 {
            self.health_timer = 0.0;
            let health = self.monitor.refresh();
            let history = self.monitor.get_history().to_vec();
            self.health_panel.update(health, history);

            for alert in self.monitor.check_throttle() {
                // Surface at most once per resource type every 60 seconds
                let now = std::time::Instant::now();
                let show = match self.alert_cooldowns.get(&alert.resource) {
                    Some(last) if now.duration_since(*last).as_secs() < 60 => false,
                    _ => true,
                };
                if show {
                    self.alert_cooldowns.insert(alert.resource.clone(), now);
                    tracing::warn!("Harness alert: {}", alert.message);
                    let severity = match alert.severity {
                        crate::system::AlertSeverity::Critical => "CRITICAL",
                        crate::system::AlertSeverity::Warning => "WARNING",
                        crate::system::AlertSeverity::Info => "INFO",
                    };
                    self.chat_panel.add_message(
                        MessageRole::System,
                        format!("HARNESS {} - {}", severity, alert.message),
                        String::new(),
                        0.0,
                        0,
                    );
                }
            }
            ctx.request_repaint();
        }

        // Poll streaming channels
        let mut stream_done = false;

        if let Some(rx) = &mut self.content_rx {
            while let Ok(token) = rx.try_recv() {
                self.chat_panel.streaming_content.push_str(&token);
                ctx.request_repaint();
            }
            if rx.is_closed() {
                stream_done = true;
            }
        }

        if let Some(rx) = &mut self.reasoning_rx {
            while let Ok(token) = rx.try_recv() {
                self.chat_panel.streaming_reasoning.push_str(&token);
                ctx.request_repaint();
            }
        }

        if let Some(rx) = &mut self.stats_rx {
            while let Ok(stats) = rx.try_recv() {
                self.chat_panel.current_tps = stats.tokens_per_second;
                self.chat_panel.last_generated_tokens = stats.generated_tokens;
                ctx.request_repaint();
            }
        }

        // Surface stream errors in the chat instead of swallowing them
        if let Some(rx) = &mut self.error_rx {
            while let Ok(err) = rx.try_recv() {
                self.chat_panel.add_message(
                    MessageRole::System,
                    format!("ERROR: {}", err),
                    String::new(),
                    0.0,
                    0,
                );
                ctx.request_repaint();
            }
        }

        if stream_done {
            self.chat_panel.finish_streaming();
            self.content_rx = None;
            self.reasoning_rx = None;
            self.stats_rx = None;
            self.error_rx = None;
            self.pending_send = false;
            ctx.request_repaint();
        }

        // Check if user sent a message
        if let Some(input) = self.chat_panel.take_input() {
            if self.ollama_connected && !self.pending_send {
                self.chat_panel.push_user_message(&input);
                self.spawn_chat(input);
            } else {
                self.chat_panel.add_message(
                    MessageRole::System,
                    "Cannot send: Ollama is offline or a response is pending.".to_string(),
                    String::new(),
                    0.0,
                    0,
                );
            }
        }

        // Apply config changes from config panel
        if self.config_panel.dirty {
            self.config = self.config_panel.config.clone();
            self.client = OllamaClient::new(&self.config.ollama);
            // Adopt the configured default only if the session pick is no
            // longer valid (e.g. user changed base_url or reset defaults).
            if !self.available_models.contains(&self.selected_model) {
                self.selected_model = self.config.ollama.model_name.clone();
            }
            self.sidebar.model_name = self.selected_model.clone();
            self.monitor.update_limits(
                self.config.resources.max_ram_mb,
                self.config.resources.max_cpu_percent,
            );
            // Sema toggles apply live: load/unload the lexicon, rewire RAG.
            if !self.config.sema.enabled {
                self.anchor = None;
                if let Some(rag) = self.rag.as_mut() {
                    rag.set_lemmatizer(None);
                }
            } else if self.anchor.is_none() {
                match sema_anchor::load_anchor(&self.config.sema.lexicon_path) {
                    Ok(a) => {
                        tracing::info!("Sema lexicon loaded: {} lemmas", a.lemma_count());
                        if self.config.sema.lemmatize_rag {
                            if let Some(rag) = self.rag.as_mut() {
                                rag.set_lemmatizer(Some(a.lexicon()));
                            }
                        }
                        self.anchor = Some(a);
                    }
                    Err(e) => tracing::warn!("Sema lexicon still unavailable: {}", e),
                }
            } else if let (Some(anchor), Some(rag)) = (&self.anchor, self.rag.as_mut()) {
                rag.set_lemmatizer(if self.config.sema.lemmatize_rag {
                    Some(anchor.lexicon())
                } else {
                    None
                });
            }
            self.config_panel.dirty = false;
        }

        // Apply font scale
        let fonts = &ctx.style().text_styles;
        // font_scale is applied dynamically

        // ---- Render layout ----
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(180.0)
            .frame(egui::Frame::NONE
                .fill(egui::Color32::from_rgb(16, 18, 22))
                .inner_margin(egui::Margin::symmetric(10i8, 8i8))
                .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(30, 35, 42))))
            .show(ctx, |ui| {
                self.sidebar.render(ui);
                self.active_panel = self.sidebar.active.clone();
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE
                .fill(egui::Color32::from_rgb(14, 16, 20))
                .inner_margin(egui::Margin::same(12i8)))
            .show(ctx, |ui| {
                match self.active_panel {
                    ActivePanel::Chat => {
                        let before = self.selected_model.clone();
                        self.chat_panel
                            .render(ui, &self.available_models, &mut self.selected_model);
                        // Model hot-swapped from the chat header: sync UI +
                        // make SAVE CONFIG persist the new default.
                        if self.selected_model != before {
                            tracing::info!("Model switched to {}", self.selected_model);
                            self.sidebar.model_name = self.selected_model.clone();
                            self.config.ollama.model_name = self.selected_model.clone();
                            self.chat_panel.add_message(
                                MessageRole::System,
                                format!("Switched model to {}", self.selected_model),
                                String::new(),
                                0.0,
                                0,
                            );
                        }
                    }
                    ActivePanel::Health => self.health_panel.render(ui),
                    ActivePanel::Config => self.config_panel.render(ui),
                    ActivePanel::Rag => self.render_rag_panel(ui),
                }
            });

        // Refresh the installed-model list on demand
        if self.chat_panel.refresh_models_requested {
            self.chat_panel.refresh_models_requested = false;
            if self.ollama_connected {
                let client2 = self.client.clone();
                match self.runtime.block_on(client2.list_models()) {
                    Ok(models) => {
                        let names: Vec<String> = models.iter().map(|m| m.name.clone()).collect();
                        self.available_models = names;
                    }
                    Err(e) => tracing::warn!("Could not refresh models: {}", e),
                }
            }
        }
    }
}

impl JerichoApp {
    fn render_rag_panel(&mut self, ui: &mut egui::Ui) {
        use egui::{Color32, RichText, Stroke, Rounding};

        ui.horizontal(|ui| {
            ui.heading(
                RichText::new("RAG // RETRIEVAL AUGMENTED GENERATION")
                    .color(Color32::from_rgb(180, 140, 255))
                    .size(16.0),
            );
        });
        ui.add_space(8.0);

        if let Some(rag) = &mut self.rag {
            let stats = rag.stats();

            egui::Frame::NONE
                .fill(Color32::from_rgb(18, 22, 28))
                .corner_radius(Rounding::same(6u8))
                .inner_margin(egui::Margin::same(12i8))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("RAG PIPELINE STATUS")
                            .color(Color32::from_rgb(180, 140, 255))
                            .strong()
                            .monospace(),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Documents: {}", stats.total_documents))
                            .color(Color32::from_rgb(200, 200, 210))
                            .monospace(),
                    );
                    ui.label(
                        RichText::new(format!("Chunks: {}", stats.total_chunks))
                            .color(Color32::from_rgb(200, 200, 210))
                            .monospace(),
                    );
                    ui.label(
                        RichText::new(format!("Tokens indexed: {}", stats.total_tokens))
                            .color(Color32::from_rgb(200, 200, 210))
                            .monospace(),
                    );
                    ui.label(
                        RichText::new(format!("Memory: {:.2} MB", stats.memory_usage_mb))
                            .color(Color32::from_rgb(200, 200, 210))
                            .monospace(),
                    );
                });

            ui.add_space(8.0);

            // Ingest controls
            egui::Frame::NONE
                .fill(Color32::from_rgb(18, 22, 28))
                .corner_radius(Rounding::same(6u8))
                .inner_margin(egui::Margin::same(12i8))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("DOCUMENT INGESTION")
                            .color(Color32::from_rgb(180, 140, 255))
                            .strong()
                            .monospace(),
                    );
                    ui.add_space(4.0);

                    for dir in &self.config.rag.document_dirs {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("Dir: {}", dir))
                                    .color(Color32::from_rgb(150, 150, 170))
                                    .monospace()
                                    .size(11.0),
                            );
                        });
                    }

                    if ui.button("Ingest Documents").clicked() {
                        let dirs: Vec<String> = self.config.rag.document_dirs.clone();
                        let exts: Vec<String> = self.config.rag.supported_extensions.clone();
                        for dir_str in &dirs {
                            let path = std::path::Path::new(dir_str);
                            match rag.ingest_directory(path, &exts) {
                                Ok(docs) => {
                                    for doc in &docs {
                                        tracing::info!(
                                            "Ingested: {} ({} chunks, {} tokens)",
                                            doc.path,
                                            doc.chunks_count,
                                            doc.total_tokens
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Ingestion error: {}", e);
                                }
                            }
                        }
                        self.sidebar.rag_stats = Some(
                            crate::gui::sidebar::RagStatsDisplay {
                                documents: rag.documents.len(),
                                chunks: rag.store.stats().total_chunks,
                                tokens: rag.store.stats().total_tokens,
                            },
                        );
                    }
                });

            ui.add_space(8.0);

            // Query test
            egui::Frame::NONE
                .fill(Color32::from_rgb(18, 22, 28))
                .corner_radius(Rounding::same(6u8))
                .inner_margin(egui::Margin::same(12i8))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("QUERY TEST")
                            .color(Color32::from_rgb(180, 140, 255))
                            .strong()
                            .monospace(),
                    );
                    ui.add_space(4.0);

                    let mut query = String::new();
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut query);
                        if ui.button("Search").clicked() && !query.is_empty() {
                            let (context, results) = rag.query(&query);
                            if context.is_empty() {
                                tracing::info!("No RAG results for: {}", query);
                            } else {
                                tracing::info!("RAG query '{}': {} results", query, results.len());
                                self.chat_panel.add_message(
                                    MessageRole::System,
                                    format!("RAG Context:\n{}", context),
                                    String::new(),
                                    0.0,
                                    0,
                                );
                            }
                        }
                    });
                });
        } else {
            ui.label(
                RichText::new("RAG is disabled. Enable it in CONFIG > RAG tab.")
                    .color(Color32::from_rgb(150, 100, 100)),
            );
        }
    }
}
