mod app;
mod config;
mod ollama;
mod rag;
mod render;
mod sema_anchor;
mod sema_synth;
mod system;
mod gui;

use tracing_subscriber::EnvFilter;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("=== PROJECT JERICHO INITIALIZING ===");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([1000.0, 600.0])
            .with_title("PROJECT JERICHO // Local AI Harness"),
        ..Default::default()
    };

    eframe::run_native(
        "project_jericho",
        options,
        Box::new(|cc| {
            let mut app = app::JerichoApp::new(cc);
            app.initialize()?;
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    )
}
