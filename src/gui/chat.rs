use egui::{self, Color32, RichText, Stroke, Vec2, Rounding};

// ============================================================
// CHAT + REASONING PANEL
// Main interaction area - shows messages, reasoning chain,
// token throughput, and streaming responses
// ============================================================

/// A single chat message in the UI
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub reasoning: String,
    pub timestamp: String,
    pub tokens_per_second: f64,
    pub token_count: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl MessageRole {
    fn label(&self) -> &str {
        match self {
            MessageRole::User => "YOU",
            MessageRole::Assistant => "JERICHO",
            MessageRole::System => "SYS",
        }
    }

    fn color(&self) -> Color32 {
        match self {
            MessageRole::User => Color32::from_rgb(100, 180, 255),
            MessageRole::Assistant => Color32::from_rgb(100, 255, 140),
            MessageRole::System => Color32::from_rgb(200, 200, 100),
        }
    }
}

/// State for the chat panel
pub struct ChatPanel {
    pub messages: Vec<ChatMessage>,
    pub input_buffer: String,
    pub is_generating: bool,
    pub streaming_content: String,
    pub streaming_reasoning: String,
    pub show_reasoning: bool,
    pub scroll_to_bottom: bool,
    pub current_tps: f64,
    pub total_tokens: u64,
    /// Set to true only when user presses Enter or clicks SEND
    pub send_requested: bool,
}

impl ChatPanel {
    pub fn new() -> Self {
        let mut messages = Vec::new();
        messages.push(ChatMessage {
            role: MessageRole::System,
            content: "Project Jericho initialized. Ollama connection pending...".to_string(),
            reasoning: String::new(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            tokens_per_second: 0.0,
            token_count: 0,
        });

        Self {
            messages,
            input_buffer: String::new(),
            is_generating: false,
            streaming_content: String::new(),
            streaming_reasoning: String::new(),
            show_reasoning: true,
            scroll_to_bottom: false,
            current_tps: 0.0,
            total_tokens: 0,
            send_requested: false,
        }
    }

    /// Render the full chat panel
    pub fn render(&mut self, ui: &mut egui::Ui) {
        // ---- Header bar ----
        ui.horizontal(|ui| {
            ui.heading(
                RichText::new("JERICHO // CHAT")
                    .color(Color32::from_rgb(100, 255, 140))
                    .size(16.0),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Token throughput indicator
                if self.current_tps > 0.0 {
                    let tps_color = if self.current_tps > 50.0 {
                        Color32::from_rgb(0, 255, 100)
                    } else if self.current_tps > 20.0 {
                        Color32::from_rgb(255, 255, 0)
                    } else {
                        Color32::from_rgb(255, 100, 100)
                    };
                    ui.label(
                        RichText::new(format!("{:.1} tok/s", self.current_tps))
                            .color(tps_color)
                            .monospace()
                            .size(12.0),
                    );
                }

                ui.separator();

                // Total tokens
                ui.label(
                    RichText::new(format!("tokens: {}", self.total_tokens))
                        .color(Color32::from_rgb(150, 150, 150))
                        .monospace()
                        .size(11.0),
                );

                ui.separator();

                // Reasoning toggle
                ui.checkbox(&mut self.show_reasoning, "Show reasoning");
            });
        });

        ui.add_space(4.0);

        // ---- Messages area (scrollable) ----
        let available_height = ui.available_height() - 80.0; // Reserve for input
        egui::ScrollArea::vertical()
            .max_height(available_height)
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for msg in &self.messages {
                    self.render_message(ui, msg);
                }

                // Streaming in-progress message
                if self.is_generating {
                    self.render_streaming(ui);
                }
            });

        ui.add_space(4.0);

        // ---- Input area ----
        ui.separator();
        ui.horizontal(|ui| {
            let input_width = ui.available_width() - 80.0;
            let response = ui.add_sized(
                [input_width, 36.0],
                egui::TextEdit::singleline(&mut self.input_buffer)
                    .hint_text(RichText::new("Type a message... (Enter to send)").italics())
                    .frame(true)
                    .margin(egui::Margin::symmetric(8i8, 6i8)),
            );

            let send_enabled = !self.input_buffer.trim().is_empty() && !self.is_generating;
            let send_btn = ui.add_enabled(
                send_enabled,
                egui::Button::new(
                    RichText::new("SEND")
                        .color(if send_enabled {
                            Color32::WHITE
                        } else {
                            Color32::GRAY
                        })
                        .monospace()
                        .size(13.0),
                )
                .fill(if send_enabled {
                    Color32::from_rgb(40, 120, 60)
                } else {
                    Color32::from_rgb(40, 40, 40)
                })
                .corner_radius(Rounding::same(4u8))
                .min_size(Vec2::new(70.0, 36.0)),
            );

            // Enter to send - check while focused OR on lost_focus (Enter causes defocus in egui)
            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter_pressed && (response.has_focus() || response.lost_focus()) {
                self.scroll_to_bottom = true;
                self.send_requested = true;
            }

            if send_btn.clicked() && send_enabled {
                self.scroll_to_bottom = true;
                self.send_requested = true;
            }
        });
    }

    fn render_message(&self, ui: &mut egui::Ui, msg: &ChatMessage) {
        let frame_color = msg.role.color();

        // Role badge + timestamp
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(msg.role.label())
                    .color(frame_color)
                    .strong()
                    .monospace()
                    .size(11.0),
            );
            ui.label(
                RichText::new(&msg.timestamp)
                    .color(Color32::from_rgb(100, 100, 100))
                    .size(10.0)
                    .monospace(),
            );
            if msg.tokens_per_second > 0.0 {
                ui.label(
                    RichText::new(format!("{:.1} tok/s", msg.tokens_per_second))
                        .color(Color32::from_rgb(120, 120, 120))
                        .size(10.0)
                        .monospace(),
                );
            }
            if msg.token_count > 0 {
                ui.label(
                    RichText::new(format!("{} tokens", msg.token_count))
                        .color(Color32::from_rgb(120, 120, 120))
                        .size(10.0)
                        .monospace(),
                );
            }
        });

        // Reasoning block (collapsible)
        if self.show_reasoning && !msg.reasoning.is_empty() {
            let reasoning_text = msg.reasoning.trim();
            if !reasoning_text.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("REASONING:")
                            .color(Color32::from_rgb(200, 160, 60))
                            .size(10.0)
                            .monospace(),
                    );
                });
                egui::Frame::NONE
                    .fill(Color32::from_rgb(20, 18, 10))
                    .corner_radius(Rounding::same(4u8))
                    .inner_margin(egui::Margin::same(8i8))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(80, 60, 20)))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(reasoning_text)
                                .color(Color32::from_rgb(180, 150, 80))
                                .monospace()
                                .size(11.0),
                        );
                    });
                ui.add_space(2.0);
            }
        }

        // Main content
        egui::Frame::NONE
            .fill(Color32::from_rgb(22, 26, 30))
            .corner_radius(Rounding::same(6u8))
            .inner_margin(egui::Margin::symmetric(10i8, 8i8))
            .stroke(Stroke::new(1.0_f32, frame_color.linear_multiply(0.3)))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(&msg.content)
                        .color(Color32::from_rgb(220, 220, 230))
                        .size(13.0),
                );
            });

        ui.add_space(6.0);
    }

    fn render_streaming(&self, ui: &mut egui::Ui) {
        // Streaming reasoning
        if self.show_reasoning && !self.streaming_reasoning.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("REASONING...")
                        .color(Color32::from_rgb(200, 160, 60))
                        .size(10.0)
                        .monospace(),
                );
                ui.spinner();
            });
            egui::Frame::NONE
                .fill(Color32::from_rgb(20, 18, 10))
                .corner_radius(Rounding::same(4u8))
                .inner_margin(egui::Margin::same(8i8))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(80, 60, 20)))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(&self.streaming_reasoning)
                            .color(Color32::from_rgb(180, 150, 80))
                            .monospace()
                            .size(11.0),
                    );
                });
        }

        // Streaming content
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("JERICHO")
                    .color(Color32::from_rgb(100, 255, 140))
                    .strong()
                    .monospace()
                    .size(11.0),
            );
            ui.spinner();
        });

        let display = if self.streaming_content.is_empty() {
            "...thinking...".to_string()
        } else {
            self.streaming_content.clone()
        };

        egui::Frame::NONE
            .fill(Color32::from_rgb(22, 26, 30))
            .corner_radius(Rounding::same(6u8))
            .inner_margin(egui::Margin::symmetric(10i8, 8i8))
            .stroke(Stroke::new(
                1.0_f32,
                Color32::from_rgb(100, 255, 140).linear_multiply(0.3),
            ))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(&display)
                        .color(Color32::from_rgb(220, 220, 230))
                        .size(13.0),
                );
            });
    }

    /// Add a completed message
    pub fn add_message(
        &mut self,
        role: MessageRole,
        content: String,
        reasoning: String,
        tps: f64,
        tokens: u64,
    ) {
        self.messages.push(ChatMessage {
            role,
            content,
            reasoning,
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            tokens_per_second: tps,
            token_count: tokens,
        });
        self.total_tokens += tokens;
        self.current_tps = tps;
    }

    /// Start streaming state
    pub fn start_streaming(&mut self) {
        self.is_generating = true;
        self.streaming_content.clear();
        self.streaming_reasoning.clear();
    }

    /// Finish streaming and move to completed message
    pub fn finish_streaming(&mut self) {
        self.is_generating = false;
        let content = self.streaming_content.clone();
        let reasoning = self.streaming_reasoning.clone();
        if !content.trim().is_empty() || !reasoning.trim().is_empty() {
            self.add_message(
                MessageRole::Assistant,
                content,
                reasoning,
                self.current_tps,
                0, // will be updated with stats
            );
        }
        self.streaming_content.clear();
        self.streaming_reasoning.clear();
    }

    /// Check if user submitted input (only when Enter pressed or SEND clicked)
    pub fn take_input(&mut self) -> Option<String> {
        if !self.send_requested || self.is_generating || self.input_buffer.trim().is_empty() {
            self.send_requested = false;
            return None;
        }
        self.send_requested = false;
        let input = self.input_buffer.trim().to_string();
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: input.clone(),
            reasoning: String::new(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            tokens_per_second: 0.0,
            token_count: 0,
        });
        self.input_buffer.clear();
        Some(input)
    }
}
