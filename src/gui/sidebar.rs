use egui::{self, Color32, RichText, Stroke, Vec2, Rounding};

// ============================================================
// SIDEBAR - Navigation + connection status + model selector
// ============================================================

#[derive(Debug, PartialEq, Clone)]
pub enum ActivePanel {
    Chat,
    Health,
    Config,
    Rag,
}

pub struct Sidebar {
    pub active: ActivePanel,
    pub ollama_connected: bool,
    pub model_name: String,
    pub rag_stats: Option<RagStatsDisplay>,
}

pub struct RagStatsDisplay {
    pub documents: usize,
    pub chunks: usize,
    pub tokens: usize,
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            active: ActivePanel::Chat,
            ollama_connected: false,
            model_name: "qwen2.5:0.5b".to_string(),
            rag_stats: None,
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(180.0);
        ui.set_max_width(180.0);

        // ---- Logo / Title ----
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("JERICHO")
                    .color(Color32::from_rgb(100, 255, 140))
                    .strong()
                    .monospace()
                    .size(22.0),
            );
            ui.label(
                RichText::new("v0.1.0")
                    .color(Color32::from_rgb(80, 80, 100))
                    .monospace()
                    .size(10.0),
            );
        });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // ---- Connection Status ----
        egui::Frame::NONE
            .fill(Color32::from_rgb(18, 22, 28))
            .corner_radius(Rounding::same(4u8))
            .inner_margin(egui::Margin::same(8i8))
            .show(ui, |ui| {
                let (color, text) = if self.ollama_connected {
                    (Color32::from_rgb(0, 200, 80), "OLLAMA: ONLINE")
                } else {
                    (Color32::from_rgb(200, 50, 50), "OLLAMA: OFFLINE")
                };
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, color);
                    ui.label(
                        RichText::new(text)
                            .color(color)
                            .monospace()
                            .size(10.0),
                    );
                });
                ui.label(
                    RichText::new(&self.model_name)
                        .color(Color32::from_rgb(120, 120, 140))
                        .monospace()
                        .size(9.0),
                );
            });

        ui.add_space(8.0);

        // ---- Navigation buttons ----
        let nav_items = [
            (ActivePanel::Chat, "CHAT", Color32::from_rgb(100, 255, 140)),
            (ActivePanel::Health, "HEALTH", Color32::from_rgb(100, 200, 255)),
            (ActivePanel::Config, "CONFIG", Color32::from_rgb(255, 180, 60)),
            (ActivePanel::Rag, "RAG", Color32::from_rgb(180, 140, 255)),
        ];

        for (panel, label, color) in &nav_items {
            let is_active = self.active == *panel;
            let bg = if is_active {
                color.linear_multiply(0.15)
            } else {
                Color32::TRANSPARENT
            };

            let btn = ui.add_sized(
                [160.0, 32.0],
                egui::Button::new(
                    RichText::new(format!("> {}", label))
                        .color(if is_active { *color } else { Color32::from_rgb(140, 140, 160) })
                        .monospace()
                        .size(13.0),
                )
                .fill(bg)
                .stroke(Stroke::new(
                    1.0_f32,
                    if is_active { color.linear_multiply(0.5) } else { Color32::from_rgb(30, 30, 40) },
                ))
                .corner_radius(Rounding::same(4u8)),
            );

            if btn.clicked() {
                self.active = panel.clone();
            }
        }

        // ---- RAG Stats ----
        if let Some(stats) = &self.rag_stats {
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(
                RichText::new("RAG STORE")
                    .color(Color32::from_rgb(180, 140, 255))
                    .strong()
                    .monospace()
                    .size(11.0),
            );
            ui.label(
                RichText::new(format!("Docs: {}", stats.documents))
                    .color(Color32::from_rgb(120, 120, 140))
                    .monospace()
                    .size(10.0),
            );
            ui.label(
                RichText::new(format!("Chunks: {}", stats.chunks))
                    .color(Color32::from_rgb(120, 120, 140))
                    .monospace()
                    .size(10.0),
            );
            ui.label(
                RichText::new(format!("Tokens: {}", stats.tokens))
                    .color(Color32::from_rgb(120, 120, 140))
                    .monospace()
                    .size(10.0),
            );
        }

        // ---- Bottom branding ----
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new("Project Jericho")
                    .color(Color32::from_rgb(50, 50, 60))
                    .monospace()
                    .size(8.0),
            );
            ui.label(
                RichText::new("Local AI Harness")
                    .color(Color32::from_rgb(40, 40, 50))
                    .monospace()
                    .size(8.0),
            );
        });
    }
}
