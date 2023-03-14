use api::chat::ChatGPT;
use eframe::egui;
use std::error::Error;
use tokio::{
    io::{self, AsyncBufReadExt},
    signal::ctrl_c,
};

mod api;
pub mod client;
pub mod error;
mod ui;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();
    // let mut stdin = io::BufReader::new(io::stdin()).lines();
    let mut chat = ChatGPT::new();
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
