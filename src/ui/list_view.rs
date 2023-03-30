use derive_more::From;
use eframe::egui;
use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
};

use strum::IntoEnumIterator;
use tokio::runtime::Handle;

use crate::api::{
    chat::{Chat, ChatAPI, ChatAPIBuilder},
    complete::{Complete, CompleteAPI, CompleteAPIBuilder},
};

use super::{chat_window::ChatWindow, complete_window::CompleteWindow, ModelType, TabWindow};

pub struct ViewContext {
    pub name: String,
    pub view: Box<dyn TabWindow<Response = ()>>,
    pub api: APIImpl,
}

#[derive(Debug, From)]
pub enum APIImpl {
    Chat(ChatAPI),
    Complete(CompleteAPI),
}

pub enum ResponseEvent {
    Select(String),
    Remove(String),
    /// from, to
    Rename(String, String),
    None,
}

impl ViewContext {
    pub fn new(name: String, api: APIImpl) -> Self {
        let view = match &api {
            APIImpl::Chat(chat) => Box::new(ChatWindow::new(name.clone(), chat.clone()))
                as Box<dyn TabWindow<Response = ()>>,
            APIImpl::Complete(complete) => {
                Box::new(CompleteWindow::new(name.clone(), complete.clone()))
            }
        };
        Self { name, view, api }
    }
}

pub struct ListView {
    text: String,
    select_mode: ModelType,
    selected: BTreeSet<String>,
    views: Vec<ViewContext>,
    rename: Option<String>,
    rename_buffer: String,
}

impl Default for ListView {
    fn default() -> Self {
        Self {
            text: String::new(),
            select_mode: ModelType::Chat,
            selected: BTreeSet::new(),
            rename: None,
            views: Vec::new(),
            rename_buffer: String::new(),
        }
    }
}

impl ListView {
    fn generate_new_name(&self) -> String {
        let mut name = String::new();
        let mut i = 1;
        loop {
            name = format!("{}_{}", self.select_mode, i);
            if !self.views.iter().any(|v| v.name == name) {
                break;
            }
            i += 1;
        }
        name
    }

    pub fn new_chat(&mut self, name: Option<String>) -> Result<(), anyhow::Error> {
        let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
        let chat = ChatAPIBuilder::new(api_key).build();
        if let Ok(system_message) = std::env::var("SYSTEM_MESSAGE") {
            if !system_message.is_empty() {
                tokio::task::block_in_place(|| {
                    Handle::current().block_on(async {
                        chat.set_system_message(Some(system_message)).await;
                    })
                });
            }
        }

        let name = name.unwrap_or_else(|| self.generate_new_name());

        let context = ViewContext::new(name.clone(), APIImpl::Chat(chat));

        self.views.push(context);
        Ok(())
    }
    pub fn new_complete(&mut self, name: Option<String>) -> Result<(), anyhow::Error> {
        let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
        let complete = CompleteAPIBuilder::new(api_key).build();
        let name = name.unwrap_or_else(|| self.generate_new_name());
        let context = ViewContext::new(name.clone(), APIImpl::Complete(complete));

        self.views.push(context);
        Ok(())
    }
    pub fn remove(&mut self, name: &str) -> Option<APIImpl> {
        self.selected.remove(name);

        let context = self
            .views
            .remove(self.views.iter().position(|v| v.name == name)?);
        Some(context.api)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error> {
        let mut save_value: HashMap<String, HashMap<String, serde_json::Value>> = HashMap::new();
        let full_path = if path.as_ref().is_dir() {
            anyhow::bail!("path is directory");
        } else {
            path.as_ref().to_path_buf()
        };
        for context in self.views.iter() {
            let name = context.name.clone();
            match &context.api {
                APIImpl::Chat(chat) => {
                    let value = serde_json::to_value(chat.data())?;
                    save_value
                        .entry("chat".to_string())
                        .and_modify(|v| {
                            v.insert(name.clone(), value.clone());
                        })
                        .or_insert_with(|| {
                            let mut map = HashMap::new();
                            map.insert(name, value);
                            map
                        });
                }
                APIImpl::Complete(complete) => {
                    let value = serde_json::to_value(complete.data())?;
                    save_value
                        .entry("complete".to_string())
                        .and_modify(|v| {
                            v.insert(name.clone(), value.clone());
                        })
                        .or_insert_with(|| {
                            let mut map = HashMap::new();
                            map.insert(name, value);
                            map
                        });
                }
            }
        }

        let mut file = std::fs::File::create(full_path)?;
        serde_json::to_writer(&mut file, &save_value)?;

        Ok(())
    }

    pub fn load<P: AsRef<Path>>(&mut self, path: P) -> Result<(), anyhow::Error> {
        let mut file = std::fs::File::open(path.as_ref())?;

        let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
        let value: HashMap<String, serde_json::Value> = serde_json::from_reader(&mut file)?;
        let chats = if let Some(value) = value.get("chat") {
            serde_json::from_value::<HashMap<String, Chat>>(value.clone())?
        } else {
            HashMap::new()
        };
        let completes = if let Some(value) = value.get("complete") {
            serde_json::from_value::<HashMap<String, Complete>>(value.clone())?
        } else {
            HashMap::new()
        };
        self.views.clear();
        self.selected.clear();
        for (name, chat) in chats {
            let chat = ChatAPIBuilder::new(api_key.clone()).with_data(chat).build();
            self.views.push(ViewContext::new(name, APIImpl::Chat(chat)));
        }
        for (name, complete) in completes {
            let complete = CompleteAPIBuilder::new(api_key.clone())
                .with_data(complete)
                .build();
            self.views
                .push(ViewContext::new(name, APIImpl::Complete(complete)));
        }

        Ok(())
    }
    pub fn action(&mut self, name: &String, ui: &mut egui::Ui) {
        if let Some(context) = self.views.iter_mut().find(|c| &c.name == name) {
            context.view.actions(ui);
        }
    }
}

impl super::View for ListView {
    type Response = ResponseEvent;

    fn ui(&mut self, ui: &mut egui::Ui) -> Self::Response {
        let mut event = ResponseEvent::None;
        let mut will_remove = None;

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
                match self.select_mode {
                    ModelType::Chat => {
                        self.new_chat(name).unwrap();
                    }
                    ModelType::Complete => {
                        self.new_complete(name).unwrap();
                    }
                    ModelType::Edit => {
                        tracing::warn!("edit mode not supported yet.")
                    }
                }
            });
            ui.menu_button("mode", |ui| {
                for t in ModelType::iter() {
                    if ui
                        .selectable_value(&mut self.select_mode, t.clone(), t.to_string())
                        .clicked()
                    {
                        ui.close_menu();
                    };
                }
            });
        });
        egui::CentralPanel::default()
            .show_inside(ui, |ui| {
                if !self.views.is_empty() {
                    egui::CollapsingHeader::new("Chat")
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.with_layout(ui.layout().with_cross_justify(true), |ui| {
                                for ViewContext { name, view, .. } in self.views.iter_mut() {
                                    if let Some(rename) = self.rename.clone() {
                                        if &rename == name {
                                            let resp =
                                                ui.text_edit_singleline(&mut self.rename_buffer);
                                            if (!self.rename_buffer.is_empty()
                                                && resp.has_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                                || (!self.rename_buffer.is_empty()
                                                    && resp.lost_focus())
                                            {
                                                self.selected.remove(name);
                                                self.selected.insert(self.rename_buffer.clone());
                                                view.set_name(self.rename_buffer.clone());
                                                event = ResponseEvent::Rename(
                                                    name.clone(),
                                                    self.rename_buffer.clone(),
                                                );
                                                *name = self.rename_buffer.clone();
                                                self.rename_buffer.clear();
                                                self.rename = None;
                                            } else {
                                                resp.request_focus();
                                            }

                                            continue;
                                        }
                                    }

                                    ui.selectable_label(
                                        self.selected.iter().find(|s| *s == name).is_some(),
                                        name.clone(),
                                    )
                                    .context_menu(|ui| {
                                        if self.rename.is_none() {
                                            if ui.button("rename").clicked() {
                                                self.rename = Some(name.clone());
                                                self.rename_buffer = name.clone();
                                                ui.close_menu();
                                            };
                                        }
                                        if ui.button("remove").clicked() {
                                            will_remove = Some(name.clone());
                                            ui.close_menu();
                                        };
                                        if ui.button("select").clicked() {
                                            self.selected.insert(name.clone());
                                            event = ResponseEvent::Select(name.clone());
                                            ui.close_menu();
                                        }
                                    })
                                    .clicked()
                                    .then(|| {
                                        self.selected.insert(name.clone());
                                        event = ResponseEvent::Select(name.clone())
                                    });
                                }
                            });
                        });
                }
            })
            .response
            .context_menu(|ui| {
                ui.label("Actions");
            });
        if let Some(name) = will_remove {
            self.remove(&name);
            event = ResponseEvent::Remove(name)
        }
        event
    }
}

impl egui_dock::TabViewer for ListView {
    type Tab = String;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        let context = self.views.iter_mut().find(|v| &v.name == tab);
        if let Some(context) = context {
            context.view.ui(ui);
        }
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        egui::WidgetText::from(&*tab)
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
        self.selected.remove(tab);
        false
    }
    fn force_close(&mut self, _tab: &mut Self::Tab) -> bool {
        !self.selected.contains(_tab)
    }
}
