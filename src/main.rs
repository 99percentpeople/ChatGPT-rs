#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![feature(is_some_and)]
#![feature(return_position_impl_trait_in_trait)]
use api::chat::{ChatAPI, ChatAPIBuilder};
use eframe::egui;
use std::error::Error;
use tracing::{info, info_span, Level};
use tracing_subscriber::prelude::*;

mod api;
mod client;
mod error;
mod ui;

use ui::logger::Logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::registry()
        .with(Logger::new(Level::TRACE))
        .init();
    // let span = tracing::span!(Level::DEBUG, "main");
    // let _enter = span.enter();

    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY is not set");
    let mut chat = ChatAPIBuilder::new(api_key).build();
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
