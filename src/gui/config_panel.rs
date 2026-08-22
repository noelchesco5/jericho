use crate::config::JerichoConfig;
use egui::{self, Color32, RichText, Stroke, Vec2, Rounding};

// ============================================================
// CONFIGURATION PANEL
// Edit all Jericho settings live: model params, resource limits,
// RAG settings, GUI preferences. Changes apply immediately.
// ============================================================

pub struct ConfigPanel {
    pub config: JerichoConfig,
    pub dirty: bool,
    pub show_advanced: bool,
    pub active_tab: ConfigTab,
    pub status_message: String,
    pub status_color: Color32,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ConfigTab {
    Model,
    Resources,
    Rag,
    Gui,
}

impl ConfigPanel {
    pub fn new(config: JerichoConfig) -> Self {
        Self {
            config,
            dirty: false,
            show_advanced: false,
            active_tab: ConfigTab::Model,
            status_message: String::new(),
            status_color: Color32::GRAY,
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(
                RichText::new("CONFIGURATION")
                    .color(Color32::from_rgb(255, 180, 60))
                    .size(16.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !self.status_message.is_empty() {
                    ui.label(
                        RichText::new(&self.status_message)
                            .color(self.status_color)
                            .monospace()
                            .size(10.0),
                    );
                }
            });
        });

        ui.add_space(4.0);

        // ---- Tab bar ----
        ui.horizontal(|ui| {
            let tabs = [
                (ConfigTab::Model, "MODEL"),
                (ConfigTab::Resources, "RESOURCES"),
                (ConfigTab::Rag, "RAG"),
                (ConfigTab::Gui, "GUI"),
            ];

            for (tab, label) in &tabs {
                let is_active = self.active_tab == *tab;
                let color = if is_active {
                    Color32::from_rgb(255, 180, 60)
                } else {
                    Color32::from_rgb(120, 120, 140)
                };

                let btn = ui.add(
                    egui::Button::new(
                        RichText::new(*label)
                            .color(color)
                            .monospace()
                            .size(12.0),
                    )
                    .fill(if is_active {
                        Color32::from_rgb(40, 35, 25)
                    } else {
                        Color32::TRANSPARENT
                    })
                    .stroke(Stroke::new(
                        1.0_f32,
                        if is_active { color } else { Color32::from_rgb(40, 40, 50) },
                    ))
                    .corner_radius(Rounding::same(4u8))
                    .min_size(Vec2::new(80.0, 28.0)),
                );

                if btn.clicked() {
                    self.active_tab = tab.clone();
                }
            }
        });

        ui.add_space(6.0);

        // ---- Tab content ----
        match self.active_tab {
            ConfigTab::Model => self.render_model_tab(ui),
            ConfigTab::Resources => self.render_resources_tab(ui),
            ConfigTab::Rag => self.render_rag_tab(ui),
            ConfigTab::Gui => self.render_gui_tab(ui),
        }

        // ---- Save button ----
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("SAVE CONFIG")
                            .color(Color32::WHITE)
                            .monospace()
                            .size(13.0),
                    )
                    .fill(if self.dirty {
                        Color32::from_rgb(60, 120, 40)
                    } else {
                        Color32::from_rgb(40, 40, 45)
                    })
                    .corner_radius(Rounding::same(4u8))
                    .min_size(Vec2::new(120.0, 30.0)),
                )
                .clicked()
            {
                self.config.save();
                self.dirty = false;
                self.status_message = "Saved.".to_string();
                self.status_color = Color32::from_rgb(0, 200, 80);
            }

            if ui
                .add(
                    egui::Button::new(
                        RichText::new("RESET DEFAULTS")
                            .color(Color32::from_rgb(200, 100, 100))
                            .monospace()
                            .size(11.0),
                    )
                    .fill(Color32::from_rgb(30, 20, 20))
                    .corner_radius(Rounding::same(4u8)),
                )
                .clicked()
            {
                self.config = JerichoConfig::default();
                self.dirty = true;
                self.status_message = "Reset to defaults.".to_string();
                self.status_color = Color32::from_rgb(255, 180, 60);
            }
        });
    }

    fn render_model_tab(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(18, 22, 28))
            .corner_radius(Rounding::same(6u8))
            .inner_margin(egui::Margin::same(12i8))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("MODEL PARAMETERS")
                        .color(Color32::from_rgb(255, 180, 60))
                        .strong()
                        .monospace(),
                );
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Model:").monospace().size(11.0));
                    ui.text_edit_singleline(&mut self.config.ollama.model_name);
                });
                ui.add_space(2.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Server URL:").monospace().size(11.0));
                    ui.text_edit_singleline(&mut self.config.ollama.base_url);
                });
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Temperature:").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.model.temperature, 0.0..=2.0).step_by(0.05));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Top-P:").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.model.top_p, 0.0..=1.0).step_by(0.05));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Top-K:").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.model.top_k, 1..=100));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Max Tokens:").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.model.num_predict, 32..=4096).step_by(32.0));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Context Size:").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.model.num_ctx, 256..=8192).step_by(256.0));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Repeat Penalty:").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.model.repeat_penalty, 1.0..=2.0).step_by(0.05));
                });

                ui.add_space(4.0);
                ui.checkbox(&mut self.config.model.thinking_mode, "Enable thinking/reasoning mode");
                ui.add_space(4.0);

                ui.label(RichText::new("System Prompt:").monospace().size(11.0));
                ui.add_sized(
                    [ui.available_width(), 60.0],
                    egui::TextEdit::multiline(&mut self.config.model.system_prompt).code_editor(),
                );
            });
    }

    fn render_resources_tab(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(18, 22, 28))
            .corner_radius(Rounding::same(6u8))
            .inner_margin(egui::Margin::same(12i8))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("RESOURCE LIMITS")
                        .color(Color32::from_rgb(100, 200, 255))
                        .strong()
                        .monospace(),
                );
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Max RAM (MB):").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.resources.max_ram_mb, 256..=4096).step_by(64.0));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Max CPU %:").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.resources.max_cpu_percent, 0.1..=1.0).step_by(0.05));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Inference Threads:").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.resources.inference_threads, 1..=8));
                });

                ui.add_space(4.0);
                ui.checkbox(&mut self.config.resources.monitor_vram, "Monitor VRAM");
                ui.checkbox(&mut self.config.resources.auto_throttle, "Auto-throttle on resource limits");

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Max VRAM (MB):").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.resources.max_vram_mb, 0..=4096).step_by(64.0));
                    if self.config.resources.max_vram_mb == 0 {
                        ui.label(RichText::new("(unlimited)").color(Color32::GRAY).monospace().size(10.0));
                    }
                });
            });
    }

    fn render_rag_tab(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(18, 22, 28))
            .corner_radius(Rounding::same(6u8))
            .inner_margin(egui::Margin::same(12i8))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("RAG PIPELINE")
                        .color(Color32::from_rgb(180, 140, 255))
                        .strong()
                        .monospace(),
                );
                ui.add_space(6.0);

                ui.checkbox(&mut self.config.rag.enabled, "Enable RAG");
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Chunk Size (words):").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.rag.chunk_size, 50..=2000).step_by(50.0));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Chunk Overlap:").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.rag.chunk_overlap, 0..=200).step_by(10.0));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Top-K Results:").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.rag.top_k_results, 1..=20));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Similarity Threshold:").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.rag.similarity_threshold, 0.0..=1.0).step_by(0.05));
                });

                ui.add_space(4.0);
                ui.label(RichText::new("Document Directories:").monospace().size(11.0));
                let mut to_remove = None;
                for (i, dir) in self.config.rag.document_dirs.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(dir);
                        if ui.small_button("X").clicked() {
                            to_remove = Some(i);
                        }
                    });
                }
                if let Some(idx) = to_remove {
                    self.config.rag.document_dirs.remove(idx);
                }
                if ui.small_button("+ Add Directory").clicked() {
                    self.config.rag.document_dirs.push("./documents".to_string());
                }

                ui.add_space(4.0);
                ui.label(RichText::new("Supported Extensions:").monospace().size(11.0));
                let exts = self.config.rag.supported_extensions.join(", ");
                ui.label(
                    RichText::new(&exts)
                        .color(Color32::from_rgb(150, 150, 170))
                        .monospace()
                        .size(10.0),
                );

                ui.add_space(8.0);
                ui.separator();
                ui.label(
                    RichText::new("SEMA (SWAHILI ANCHORING)")
                        .color(Color32::from_rgb(255, 200, 100))
                        .strong()
                        .monospace(),
                );
                ui.add_space(4.0);
                ui.checkbox(&mut self.config.sema.enabled, "Anchor Swahili input before inference");
                ui.checkbox(&mut self.config.sema.lemmatize_rag, "Lemmatize RAG tokens with Sema");
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Lexicon:").monospace().size(11.0));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.config.sema.lexicon_path)
                            .desired_width(280.0),
                    );
                });
                ui.label(
                    RichText::new("Data: Wiktionary via kaikki.org - CC BY-SA 4.0 (see data/NOTICE)")
                        .color(Color32::GRAY)
                        .monospace()
                        .size(10.0),
                );
                ui.label(
                    RichText::new("Tip: use qwen2.5:1.5b+ for Swahili replies. 0.5b echoes input.")
                        .color(Color32::from_rgb(255, 180, 80))
                        .monospace()
                        .size(10.0),
                );
            });
    }

    fn render_gui_tab(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(18, 22, 28))
            .corner_radius(Rounding::same(6u8))
            .inner_margin(egui::Margin::same(12i8))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("GUI SETTINGS")
                        .color(Color32::from_rgb(100, 255, 140))
                        .strong()
                        .monospace(),
                );
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Stats Refresh (ms):").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.gui.stats_refresh_ms, 100..=5000).step_by(100.0));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Font Scale:").monospace().size(11.0));
                    ui.add(egui::Slider::new(&mut self.config.gui.font_scale, 0.5..=2.0).step_by(0.1));
                });

                ui.checkbox(&mut self.config.gui.show_token_speed, "Show token throughput");
                ui.checkbox(&mut self.config.gui.show_reasoning, "Show reasoning chain");

                ui.add_space(4.0);
                ui.label(RichText::new("Theme:").monospace().size(11.0));
                ui.horizontal(|ui| {
                    for theme in &["dark", "hacker", "light"] {
                        let is_selected = self.config.gui.theme == *theme;
                        if ui
                            .selectable_label(
                                is_selected,
                                RichText::new(*theme).monospace().size(11.0),
                            )
                            .clicked()
                        {
                            self.config.gui.theme = theme.to_string();
                            self.dirty = true;
                        }
                    }
                });
            });
    }
}
