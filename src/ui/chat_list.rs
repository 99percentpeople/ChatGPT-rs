use std::collections::HashMap;

use eframe::egui;

use strum::IntoEnumIterator;
use tokio::runtime::Handle;

use crate::api::{
    chat::{Chat, ChatAPI, ChatAPIBuilder},
    complete::{Complete, CompleteAPI, CompleteAPIBuilder},
};

use super::{chat_window::ChatWindow, complete_window::CompleteWindow, MainWindow, ModelType};

pub enum ResponseEvent<'a> {
    Select(Box<dyn MainWindow + 'a>),
    Remove,
    None,
}

pub struct ChatList {
    chat_list: HashMap<String, ChatAPI>,
    complete_list: HashMap<String, CompleteAPI>,
    text: String,
    select_type: ModelType,
}

impl Default for ChatList {
    fn default() -> Self {
        Self {
            chat_list: HashMap::new(),
            complete_list: HashMap::new(),
            text: String::new(),
            select_type: ModelType::Chat,
        }
    }
}

impl ChatList {
    pub fn new_chat(
        &mut self,
        name: Option<String>,
    ) -> Result<Box<dyn MainWindow + '_>, anyhow::Error> {
        let api_key = std::env::var("OPENAI_API_KEY")?;
        let mut chat = ChatAPIBuilder::new(api_key).build();
        if let Ok(system_message) = std::env::var("SYSTEM_MESSAGE") {
            if !system_message.is_empty() {
                tokio::task::block_in_place(|| {
                    Handle::current().block_on(async {
                        chat.system(system_message).await;
                    })
                });
            }
        }
        let name = name.unwrap_or_else(|| format!("chat_{}", self.chat_list.len() + 1));
        self.chat_list.insert(name.to_owned(), chat);
        Ok(Box::new(ChatWindow::new(
            self.chat_list.get(&name).unwrap().clone(),
        )))
    }
    pub fn new_complete(
        &mut self,
        name: Option<String>,
    ) -> Result<Box<dyn MainWindow + '_>, anyhow::Error> {
        let api_key = std::env::var("OPENAI_API_KEY")?;
        let complete = CompleteAPIBuilder::new(api_key).build();
        let name = name.unwrap_or_else(|| format!("complete_{}", self.complete_list.len() + 1));
        self.complete_list.insert(name.to_owned(), complete);
        Ok(Box::new(CompleteWindow::new(
            self.complete_list.get_mut(&name).unwrap().clone(),
        )))
    }
    pub fn remove_chat(&mut self, name: &str) -> Option<ChatAPI> {
        self.chat_list.remove(name)
    }
    pub fn remove_complete(&mut self, name: &str) -> Option<CompleteAPI> {
        self.complete_list.remove(name)
    }
    pub fn save(&self) -> Result<(), anyhow::Error> {
        let mut file = std::fs::File::create("chats.json")?;
        let value = tokio::task::block_in_place(|| {
            let mut chat = HashMap::new();
            let mut complete = HashMap::new();
            Handle::current().block_on(async {
                for (name, c) in self.chat_list.iter() {
                    chat.insert(name, c.data.read().await.clone());
                }
                for (name, c) in self.complete_list.iter() {
                    complete.insert(name, c.complete.read().await.clone());
                }
            });
            <Result<_, anyhow::Error>>::Ok(HashMap::from([
                ("chat", serde_json::to_value(chat)?),
                ("complete", serde_json::to_value(complete)?),
            ]))
        })?;
        serde_json::to_writer(&mut file, &value)?;
        Ok(())
    }
    pub fn load(&mut self) -> Result<(), anyhow::Error> {
        let api_key = std::env::var("OPENAI_API_KEY")?;
        let mut file = std::fs::File::open("chats.json")?;
        let value: HashMap<String, serde_json::Value> = serde_json::from_reader(&mut file)?;
        tokio::task::block_in_place(|| {
            let chats = serde_json::from_value::<HashMap<String, Chat>>(
                value.get("chat").ok_or(anyhow::anyhow!(""))?.clone(),
            )?;
            let completes = serde_json::from_value::<HashMap<String, Complete>>(
                value.get("complete").ok_or(anyhow::anyhow!(""))?.clone(),
            )?;
            Handle::current().block_on(async {
                for (name, chat) in chats {
                    self.chat_list.insert(
                        name,
                        ChatAPIBuilder::new(api_key.clone()).with_chat(chat).build(),
                    );
                }
                for (name, complete) in completes {
                    self.complete_list.insert(
                        name,
                        CompleteAPIBuilder::new(api_key.clone())
                            .with_complete(complete)
                            .build(),
                    );
                }
            });
            <Result<(), anyhow::Error>>::Ok(())
        })?;
        Ok(())
    }
}

impl super::View for ChatList {
    type Response<'a> = ResponseEvent<'a>;

    fn ui(&mut self, ui: &mut egui::Ui) -> Self::Response<'_> {
        let mut event = ResponseEvent::None;
        let mut remove_chat = None;
        let mut remove_complete = None;
        ui.horizontal(|ui| {
            ui.add_sized(
                [100., ui.available_height()],
                egui::TextEdit::singleline(&mut self.text).desired_width(f32::INFINITY),
            );
            ui.button("new").clicked().then(|| {
                let name = if self.text.is_empty() {
                    None
                } else {
                    let name = Some(self.text.clone());
                    self.text.clear();
                    name
                };
                match self.select_type {
                    ModelType::Chat => {
                        self.new_chat(name).unwrap();
                    }
                    ModelType::Complete => {
                        self.new_complete(name).unwrap();
                    }
                    _ => {}
                }
            });
            ui.menu_button("type", |ui| {
                for t in ModelType::iter() {
                    ui.selectable_value(&mut self.select_type, t.clone(), t.to_string());
                }
            });
        });
        egui::TopBottomPanel::bottom("list_bottom").show_inside(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.button("Save").clicked().then(|| {
                    if let Err(e) = self.save() {
                        tracing::error!("Error saving chats: {}", e);
                    }
                });
                ui.button("Load").clicked().then(|| {
                    if let Err(e) = self.load() {
                        tracing::error!("Error loading chats: {}", e);
                    }
                });
            });
        });
        egui::CollapsingHeader::new("Chat")
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("list").striped(true).show(ui, |ui| {
                    ui.label("Name");
                    ui.label("Action");
                    ui.end_row();

                    for (name, chat) in self.chat_list.iter() {
                        ui.label(name);
                        if ui.button("remove").clicked() {
                            remove_chat = Some(name.clone());
                        };
                        if ui.button("select").clicked() {
                            event = ResponseEvent::Select(Box::new(ChatWindow::new(chat.clone())));
                        }

                        ui.end_row();
                    }
                });
            });
        egui::CollapsingHeader::new("Complete")
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("list").striped(true).show(ui, |ui| {
                    ui.label("Name");
                    ui.label("Action");
                    ui.end_row();

                    for (name, complete) in self.complete_list.iter() {
                        ui.label(name);
                        if ui.button("remove").clicked() {
                            remove_complete = Some(name.clone());
                        };
                        if ui.button("select").clicked() {
                            event = ResponseEvent::Select(Box::new(CompleteWindow::new(
                                complete.clone(),
                            )));
                        }

                        ui.end_row();
                    }
                });
            });
        if let Some(name) = remove_chat {
            self.remove_chat(&name);
            return ResponseEvent::Remove;
        }
        if let Some(name) = remove_complete {
            self.remove_complete(&name);
            return ResponseEvent::Remove;
        }
        event
    }
}
