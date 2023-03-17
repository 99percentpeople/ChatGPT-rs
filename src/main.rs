#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![feature(is_some_and)]
use api::chat::ChatAPI;
use eframe::egui;
use std::error::Error;
use tracing::{info, info_span, Level};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

mod api;
pub mod client;
pub mod error;
mod ui;
use ui::logger::Logger;
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();

    tracing_subscriber::registry()
        .with(Logger::new(Level::TRACE))
        .init();
    let span1 = info_span!("main0", level = 1);
    let _entered = span1.enter();
    let span2 = info_span!("main1", level = 2);
    let _entered1 = span2.enter();
    info!(a_bool = true, answer = 42, message = "first example");

    let mut chat = ChatAPI::new();
    if let Ok(system_message) = std::env::var("SYSTEM_MESSAGE") {
        if !system_message.is_empty() {
            chat.system(system_message).await;
        }
    }

    let local = tokio::task::LocalSet::new();
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1024.0, 768.0)),
        ..Default::default()
    };
    local.spawn_local(async move {
        eframe::run_native(
            "Chat App",
            options,
            Box::new(|cc| Box::new(ui::ChatApp::new_with_chat(&cc, chat))),
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok::<(), anyhow::Error>(())
    });

    local.await;
    Ok(())
}
