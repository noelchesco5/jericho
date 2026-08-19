use crate::system::{SystemHealth, HealthSnapshot, AlertSeverity};
use egui::{self, Color32, RichText, Stroke, Vec2, Rounding, Rect};

// ============================================================
// SYSTEM HEALTH DASHBOARD
// Live-updating graphs and stats for RAM, CPU, disk, process,
// GPU (if available), Ollama process, and throttle alerts
// ============================================================

pub struct HealthPanel {
    pub current: Option<SystemHealth>,
    pub history: Vec<HealthSnapshot>,
    pub show_per_core: bool,
    pub show_alerts: bool,
}

impl HealthPanel {
    pub fn new() -> Self {
        Self {
            current: None,
            history: Vec::new(),
            show_per_core: false,
            show_alerts: true,
        }
    }

    pub fn update(&mut self, health: SystemHealth, history: Vec<HealthSnapshot>) {
        self.current = Some(health);
        self.history = history;
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(
                RichText::new("SYSTEM HEALTH")
                    .color(Color32::from_rgb(100, 200, 255))
                    .size(16.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut self.show_per_core, "Per-core CPU");
            });
        });
        ui.add_space(4.0);

        let health = match &self.current {
            Some(h) => h.clone(),
            None => {
                ui.label(RichText::new("Waiting for first system sample...").color(Color32::GRAY));
                return;
            }
        };

        // ---- RAM Section ----
        self.render_ram_section(ui, &health);

        ui.add_space(6.0);

        // ---- CPU Section ----
        self.render_cpu_section(ui, &health);

        ui.add_space(6.0);

        // ---- CPU History Graph ----
        self.render_cpu_graph(ui);

        ui.add_space(6.0);

        // ---- RAM History Graph ----
        self.render_ram_graph(ui);

        ui.add_space(6.0);

        // ---- Disk Section ----
        self.render_disk_section(ui, &health);

        ui.add_space(6.0);

        // ---- Jericho Process ----
        self.render_process_section(ui, &health);

        ui.add_space(6.0);

        // ---- Ollama Process ----
        self.render_ollama_section(ui, &health);

        ui.add_space(6.0);

        // ---- Alerts ----
        if self.show_alerts {
            self.render_alerts_section(ui, &health);
        }
    }

    fn render_ram_section(&self, ui: &mut egui::Ui, health: &SystemHealth) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(18, 22, 28))
            .corner_radius(Rounding::same(6u8))
            .inner_margin(egui::Margin::same(10i8))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("RAM")
                            .color(Color32::from_rgb(100, 200, 255))
                            .strong()
                            .monospace()
                            .size(13.0),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{:.1}%  ({}/{})",
                            health.ram.usage_percent,
                            health.ram.used_mb,
                            health.ram.total_mb
                        ))
                        .color(Color32::from_rgb(200, 200, 210))
                        .monospace()
                        .size(12.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("JERICHO: {}MB", health.ram.jericho_used_mb))
                                .color(Color32::from_rgb(100, 255, 140))
                                .monospace()
                                .size(11.0),
                        );
                    });
                });

                ui.add_space(4.0);
                // RAM bar
                let bar_width = ui.available_width();
                let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, 16.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, Rounding::same(3u8), Color32::from_rgb(30, 30, 40));
                let fill_width = bar_width * (health.ram.usage_percent / 100.0);
                let fill_rect = Rect::from_min_size(rect.min, Vec2::new(fill_width, 16.0));
                let bar_color = if health.ram.usage_percent > 90.0 {
                    Color32::from_rgb(200, 50, 50)
                } else if health.ram.usage_percent > 75.0 {
                    Color32::from_rgb(200, 150, 50)
                } else {
                    Color32::from_rgb(50, 140, 200)
                };
                ui.painter().rect_filled(fill_rect, Rounding::same(3u8), bar_color);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Free: {}MB | Available: {}MB", health.ram.free_mb, health.ram.available_mb))
                            .color(Color32::from_rgb(120, 120, 140))
                            .monospace()
                            .size(10.0),
                    );
                });
            });
    }

    fn render_cpu_section(&self, ui: &mut egui::Ui, health: &SystemHealth) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(18, 22, 28))
            .corner_radius(Rounding::same(6u8))
            .inner_margin(egui::Margin::same(10i8))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("CPU")
                            .color(Color32::from_rgb(255, 200, 100))
                            .strong()
                            .monospace()
                            .size(13.0),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{:.1}%  |  {} cores  |  {}MHz  |  {}",
                            health.cpu.usage_percent,
                            health.cpu.core_count,
                            health.cpu.frequency_mhz,
                            health.cpu.brand,
                        ))
                        .color(Color32::from_rgb(200, 200, 210))
                        .monospace()
                        .size(11.0),
                    );
                });

                // Per-core bars
                if self.show_per_core && !health.cpu.per_core_usage.is_empty() {
                    ui.add_space(4.0);
                    for (i, usage) in health.cpu.per_core_usage.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("C{i:02}"))
                                    .color(Color32::from_rgb(100, 100, 120))
                                    .monospace()
                                    .size(9.0),
                            );
                            let bar_width = ui.available_width() - 50.0;
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, 8.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, Rounding::same(2u8), Color32::from_rgb(30, 30, 40));
                            let fill_w = bar_width * (usage / 100.0);
                            let fill_rect = Rect::from_min_size(rect.min, Vec2::new(fill_w, 8.0));
                            let color = if *usage > 90.0 {
                                Color32::from_rgb(200, 50, 50)
                            } else if *usage > 60.0 {
                                Color32::from_rgb(200, 150, 50)
                            } else {
                                Color32::from_rgb(100, 200, 100)
                            };
                            ui.painter().rect_filled(fill_rect, Rounding::same(2u8), color);
                            ui.label(
                                RichText::new(format!("{usage:.0}%"))
                                    .color(Color32::from_rgb(140, 140, 160))
                                    .monospace()
                                    .size(9.0),
                            );
                        });
                    }
                }
            });
    }

    fn render_cpu_graph(&self, ui: &mut egui::Ui) {
        if self.history.is_empty() {
            return;
        }
        self.render_mini_graph(
            ui,
            "CPU HISTORY",
            Color32::from_rgb(255, 200, 100),
            &self.history,
            |s| s.cpu_percent,
        );
    }

    fn render_ram_graph(&self, ui: &mut egui::Ui) {
        if self.history.is_empty() {
            return;
        }
        self.render_mini_graph(
            ui,
            "RAM HISTORY",
            Color32::from_rgb(100, 200, 255),
            &self.history,
            |s| s.ram_percent,
        );
    }

    fn render_mini_graph(
        &self,
        ui: &mut egui::Ui,
        title: &str,
        color: Color32,
        data: &[HealthSnapshot],
        extractor: impl Fn(&HealthSnapshot) -> f32,
    ) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(18, 22, 28))
            .corner_radius(Rounding::same(6u8))
            .inner_margin(egui::Margin::same(8i8))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(title)
                        .color(color)
                        .strong()
                        .monospace()
                        .size(11.0),
                );
                ui.add_space(2.0);

                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 60.0),
                    egui::Sense::hover(),
                );

                // Background grid
                let painter = ui.painter();
                painter.rect_filled(rect, Rounding::same(3u8), Color32::from_rgb(12, 14, 18));

                // Draw grid lines
                for i in 1..5 {
                    let y = rect.min.y + rect.height() * (i as f32 / 5.0);
                    painter.line_segment(
                        [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                        Stroke::new(0.5_f32, Color32::from_rgb(25, 30, 35)),
                    );
                }

                // Draw line graph
                if data.len() > 1 {
                    let points: Vec<egui::Pos2> = data
                        .iter()
                        .enumerate()
                        .map(|(i, d)| {
                            let x = rect.min.x + rect.width() * (i as f32 / (data.len() - 1) as f32);
                            let y = rect.max.y - rect.height() * (extractor(d) / 100.0);
                            egui::pos2(x, y)
                        })
                        .collect();

                    // Fill under curve
                    let mut fill_points = points.clone();
                    fill_points.push(egui::pos2(rect.max.x, rect.max.y));
                    fill_points.push(egui::pos2(rect.min.x, rect.max.y));
                    painter.add(egui::Shape::convex_polygon(
                        fill_points,
                        color.linear_multiply(0.15),
                        Stroke::NONE,
                    ));

                    // Line
                    for pair in points.windows(2) {
                        painter.line_segment(
                            [pair[0], pair[1]],
                            Stroke::new(1.5_f32, color),
                        );
                    }
                }
            });
    }

    fn render_disk_section(&self, ui: &mut egui::Ui, health: &SystemHealth) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(18, 22, 28))
            .corner_radius(Rounding::same(6u8))
            .inner_margin(egui::Margin::same(10i8))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("DISK")
                            .color(Color32::from_rgb(180, 140, 255))
                            .strong()
                            .monospace()
                            .size(13.0),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{:.1}%  |  {:.1}GB / {:.1}GB  |  Free: {:.1}GB",
                            health.disk.usage_percent,
                            health.disk.used_gb,
                            health.disk.total_gb,
                            health.disk.free_gb,
                        ))
                        .color(Color32::from_rgb(200, 200, 210))
                        .monospace()
                        .size(11.0),
                    );
                });
            });
    }

    fn render_process_section(&self, ui: &mut egui::Ui, health: &SystemHealth) {
        egui::Frame::NONE
            .fill(Color32::from_rgb(18, 22, 28))
            .corner_radius(Rounding::same(6u8))
            .inner_margin(egui::Margin::same(10i8))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("JERICHO PROCESS")
                        .color(Color32::from_rgb(100, 255, 140))
                        .strong()
                        .monospace()
                        .size(13.0),
                );
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "PID: {}  |  RAM: {}MB  |  CPU: {:.1}%  |  Threads: {}  |  Uptime: {}s",
                            health.process.pid,
                            health.process.memory_mb,
                            health.process.cpu_percent,
                            health.process.thread_count,
                            health.process.uptime_secs,
                        ))
                        .color(Color32::from_rgb(200, 200, 210))
                        .monospace()
                        .size(11.0),
                    );
                });
            });
    }

    fn render_ollama_section(&self, ui: &mut egui::Ui, health: &SystemHealth) {
        let ollama = &health.ollama_process;
        egui::Frame::NONE
            .fill(Color32::from_rgb(18, 22, 28))
            .corner_radius(Rounding::same(6u8))
            .inner_margin(egui::Margin::same(10i8))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 60, 80)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (status_color, status_text) = if ollama.as_ref().map_or(false, |o| o.running) {
                        (Color32::from_rgb(0, 200, 80), "RUNNING")
                    } else {
                        (Color32::from_rgb(200, 50, 50), "STOPPED")
                    };

                    ui.label(
                        RichText::new("OLLAMA")
                            .color(Color32::from_rgb(255, 160, 60))
                            .strong()
                            .monospace()
                            .size(13.0),
                    );
                    ui.label(
                        RichText::new(status_text)
                            .color(status_color)
                            .monospace()
                            .size(12.0),
                    );
                });

                if let Some(o) = ollama {
                    if o.running {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "PID: {}  |  RAM: {}MB  |  CPU: {:.1}%  |  Threads: {}",
                                    o.pid.map_or(0, |p| p),
                                    o.memory_mb,
                                    o.cpu_percent,
                                    o.thread_count,
                                ))
                                .color(Color32::from_rgb(200, 200, 210))
                                .monospace()
                                .size(11.0),
                            );
                        });
                    }
                }
            });
    }

    fn render_alerts_section(&self, ui: &mut egui::Ui, health: &SystemHealth) {
        let mut monitor = crate::system::SystemMonitor::new(2048, 0.8);
        let alerts = monitor.check_throttle();

        if !alerts.is_empty() {
            egui::Frame::NONE
                .fill(Color32::from_rgb(40, 18, 18))
                .corner_radius(Rounding::same(6u8))
                .inner_margin(egui::Margin::same(10i8))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(120, 40, 40)))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("ALERTS")
                            .color(Color32::from_rgb(255, 80, 80))
                            .strong()
                            .monospace()
                            .size(13.0),
                    );
                    for alert in &alerts {
                        let color = match alert.severity {
                            AlertSeverity::Critical => Color32::from_rgb(255, 50, 50),
                            AlertSeverity::Warning => Color32::from_rgb(255, 180, 50),
                            AlertSeverity::Info => Color32::from_rgb(100, 200, 255),
                        };
                        ui.label(
                            RichText::new(format!("[{:?}] {}", alert.severity, alert.message))
                                .color(color)
                                .monospace()
                                .size(11.0),
                        );
                    }
                });
        }
    }
}
