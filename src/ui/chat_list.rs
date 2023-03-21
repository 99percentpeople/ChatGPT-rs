use std::collections::HashMap;

use eframe::egui;
use tokio::runtime::Handle;

use crate::api::chat::{Chat, ChatAPI, ChatAPIBuilder};

pub enum ResponseEvent<'a> {
    SelectChat(&'a mut ChatAPI),
    RemoveChat,
    None,
}

pub struct ChatList {
    list: HashMap<String, ChatAPI>,
    text: String,
}

impl Default for ChatList {
    fn default() -> Self {
        Self {
            list: HashMap::new(),
            text: String::new(),
        }
    }
}

impl ChatList {
    pub fn new_chat(&mut self, name: Option<String>) -> Result<&mut ChatAPI, anyhow::Error> {
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
        let name = name.unwrap_or_else(|| format!("topic_{}", self.list.len() + 1));
        self.list.insert(name.to_owned(), chat);
        Ok(self.list.get_mut(&name).unwrap())
    }
    pub fn remove_chat(&mut self, name: &str) -> Option<ChatAPI> {
        self.list.remove(name)
    }
    pub fn save(&self) -> Result<(), anyhow::Error> {
        let mut file = std::fs::File::create("chats.json")?;
        let value = tokio::task::block_in_place(|| {
            let mut value = HashMap::new();
            Handle::current().block_on(async {
                for (name, chat) in self.list.iter() {
                    value.insert(name, chat.chat.read().await.clone());
                }
            });
            value
        });
        serde_json::to_writer(&mut file, &value)?;
        Ok(())
    }
    pub fn load(&mut self) -> Result<(), anyhow::Error> {
        let api_key = std::env::var("OPENAI_API_KEY")?;
        let mut file = std::fs::File::open("chats.json")?;
        let value: HashMap<String, Chat> = serde_json::from_reader(&mut file)?;
        tokio::task::block_in_place(|| {
            Handle::current().block_on(async {
                for (name, chat) in value {
                    self.list.insert(
                        name,
                        ChatAPIBuilder::new(api_key.clone()).with_chat(chat).build(),
                    );
                }
            });
        });
        Ok(())
    }
}

impl super::View for ChatList {
    type Response<'a> = ResponseEvent<'a>;

    fn ui(&mut self, ui: &mut egui::Ui) -> Self::Response<'_> {
        let mut select_chat = None;
        let mut remove_chat = None;

        egui::Grid::new("list").striped(true).show(ui, |ui| {
            ui.label("Name");
            ui.label("Action");
            ui.end_row();
            ui.add_sized(
                [100., ui.available_height()],
                egui::TextEdit::singleline(&mut self.text).desired_width(f32::INFINITY),
            );

            ui.button("new").clicked().then(|| {
                if !self.text.is_empty() {
                    self.new_chat(Some(self.text.clone())).unwrap();
                    self.text.clear();
                } else {
                    self.new_chat(None).unwrap();
                }
            });

            ui.end_row();
            for name in self.list.keys() {
                ui.label(name);
                if ui.button("remove").clicked() {
                    remove_chat = Some(name.clone());
                };
                if ui.button("select").clicked() {
                    select_chat = Some(name.clone());
                }

                ui.end_row();
            }
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
        if let Some(name) = remove_chat {
            self.remove_chat(&name);
            return ResponseEvent::RemoveChat;
        }
        if let Some(name) = select_chat {
            self.list
                .get_mut(&name)
                .map_or(ResponseEvent::None, |c| ResponseEvent::SelectChat(c))
        } else {
            ResponseEvent::None
        }
    }
}
