#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![feature(is_some_and)]
#![feature(fn_traits)]
#![feature(specialization)]
#![feature(panic_info_message)]
#![feature(return_position_impl_trait_in_trait)]
use eframe::egui;
use std::error::Error;
use std::{fs, io::Write, panic};
use tracing::Level;
use tracing_subscriber::prelude::*;
mod api;
mod client;
mod ui;

use ui::logger::Logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    panic::set_hook(Box::new(|panic_info| {
        eprintln!("{panic_info}");
        if let Ok(f) = fs::File::create("panic.log") {
            let mut f = std::io::BufWriter::new(f);
            writeln!(f, "{panic_info}").ok();
        }
    }));

    dotenv::dotenv().ok();
    tracing_subscriber::registry()
        .with(Logger::new(Level::TRACE))
        .init();

    let local = tokio::task::LocalSet::new();
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1024.0, 768.0)),
        follow_system_theme: true,
        ..Default::default()
    };
    local.spawn_local(async move {
        eframe::run_native(
            "ChatGPT-rs",
            options,
            Box::new(|cc| Box::new(ui::ChatApp::new(cc))),
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok::<(), anyhow::Error>(())
    });

    local.await;
    Ok(())
}
