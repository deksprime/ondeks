//! Ondeks DAW - Desktop Application

use anyhow::Result;
use eframe::egui;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod app;

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "ondeks=info,eframe=warn".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Ondeks DAW");

    // Configure native options
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Ondeks DAW"),
        ..Default::default()
    };

    // Run the application
    eframe::run_native(
        "Ondeks DAW",
        options,
        Box::new(|cc| Ok(Box::new(app::OndeksApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run application: {}", e))
}
