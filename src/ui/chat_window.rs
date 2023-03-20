use crate::api::chat::{ChatAPI, Role};
use eframe::{egui, epaint};
use egui_notify::Toasts;
use std::{
    sync::{atomic, Arc},
    time::Duration,
};
use tokio::task::JoinHandle;

use super::{model_table::ModelTable, parameter_control::ParameterControl, View};

pub struct ChatWindow {
    chatgpt: ChatAPI,
    text: String,
    complete_handle: Option<JoinHandle<()>>,
    error_message: Option<String>,
    is_ready: Arc<atomic::AtomicBool>,
    show_model_table: bool,
    show_parameter_control: bool,
    model_table: ModelTable,
    parameter_control: ParameterControl,
    toasts: Toasts,
}

impl ChatWindow {
    pub fn new(chatgpt: ChatAPI) -> Self {
        let mut model_table = ModelTable::default();
        {
            let chat = chatgpt.clone();
            model_table.on_select_model(move |model| {
                let mut chat = chat.clone();
                tokio::spawn(async move {
                    chat.set_model(model).await;
                });
            });
        }
        let mut parameter_control = ParameterControl::default();
        {
            let chat = chatgpt.clone();
            parameter_control.on_max_tokens_changed(move |max_tokens| {
                let mut chat = chat.clone();
                tokio::spawn(async move {
                    chat.set_max_tokens(max_tokens).await;
                });
            });
        }
        {
            let chat = chatgpt.clone();
            parameter_control.on_temperature_changed(move |temperature| {
                let mut chat = chat.clone();
                tokio::spawn(async move {
                    chat.set_temperature(temperature).await;
                });
            });
        }
        {
            let chat = chatgpt.clone();
            parameter_control.on_top_p_changed(move |top_p| {
                let mut chat = chat.clone();
                tokio::spawn(async move {
                    chat.set_top_p(top_p).await;
                });
            });
        }
        {
            let chat = chatgpt.clone();
            parameter_control.on_presence_penalty_changed(move |presence_penalty| {
                let mut chat = chat.clone();
                tokio::spawn(async move {
                    chat.set_presence_penalty(presence_penalty).await;
                });
            });
        }
        {
            let chat = chatgpt.clone();
            parameter_control.on_frequency_penalty_changed(move |frequency_penalty| {
                let mut chat = chat.clone();
                tokio::spawn(async move {
                    chat.set_frequency_penalty(frequency_penalty).await;
                });
            });
        }
        Self {
            chatgpt,
            text: String::new(),
            toasts: Toasts::default(),
            complete_handle: None,
            model_table,
            error_message: None,
            show_model_table: false,
            show_parameter_control: false,
            is_ready: Arc::new(atomic::AtomicBool::new(true)),
            parameter_control,
        }
    }
}

impl super::MainWindow for ChatWindow {
    fn name(&self) -> &'static str {
        "Chat"
    }
    fn show(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.ui(ui);
        });
    }
    fn actions(&mut self, ui: &mut egui::Ui) {
        ui.selectable_label(self.show_model_table, "Model Table")
            .clicked()
            .then(|| {
                self.show_model_table = !self.show_model_table;
            });
        ui.selectable_label(self.show_parameter_control, "Parameter Control")
            .clicked()
            .then(|| {
                self.show_parameter_control = !self.show_parameter_control;
            });
    }
}

impl super::View for ChatWindow {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let (chat, generate) = {
            let chatgpt = &self.chatgpt;
            let pending_generate = &chatgpt.pending_generate;
            let chat = &chatgpt.chat;
            tokio::task::block_in_place(move || {
                (
                    chat.blocking_read().clone(),
                    if let Some(pending_generate) = pending_generate.blocking_read().as_ref() {
                        match pending_generate {
                            Ok(pending_generate) => pending_generate.content.clone(),
                            Err(e) => Some(e.to_string()),
                        }
                    } else {
                        None
                    },
                )
            })
        };
        let is_ready = self.is_ready.load(atomic::Ordering::Relaxed);
        let ready_to_retry = chat
            .messages
            .last()
            .is_some_and(|msg| msg.role == Role::User)
            && is_ready;
        if is_ready {
            self.complete_handle.take();
        }
        egui::SidePanel::left("left_panel").show_animated_inside(ui, self.show_model_table, |ui| {
            self.model_table.ui(ui);
        });

        egui::SidePanel::right("right_panel").show_animated_inside(
            ui,
            self.show_parameter_control,
            |ui| {
                self.parameter_control.ui(ui);
            },
        );

        egui::TopBottomPanel::bottom("bottom_panel").show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                ui.add_enabled_ui(is_ready, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.text).desired_width(f32::INFINITY));
                });
                ui.add_space(5.);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(is_ready, |ui| {
                        ui.add_sized(egui::vec2(50., 40.), egui::Button::new("Send"))
                            .clicked()
                            .then(|| {
                                let input_text = self.text.trim().to_string();
                                if !input_text.is_empty() {
                                    let mut chat = self.chatgpt.clone();
                                    let is_ready = self.is_ready.clone();
                                    self.complete_handle.replace(tokio::spawn(async move {
                                        is_ready.store(false, atomic::Ordering::Relaxed);
                                        chat.question(input_text).await.ok();
                                        is_ready.store(true, atomic::Ordering::Relaxed);
                                    }));
                                    self.text.clear();
                                }
                            });

                        ui.add_sized(egui::vec2(50., 40.), egui::Button::new("Clear"))
                            .clicked()
                            .then(|| {
                                let mut chat = self.chatgpt.clone();
                                tokio::spawn(async move {
                                    chat.clear_message().await;
                                });
                            });
                    });
                    if self.complete_handle.is_some() {
                        ui.add_sized(egui::vec2(50., 40.), egui::Button::new("Abort"))
                            .clicked()
                            .then(|| {
                                self.complete_handle.take().unwrap().abort();
                                self.is_ready.store(true, atomic::Ordering::Relaxed);
                            });
                    }
                    if ready_to_retry {
                        ui.add_sized(egui::vec2(50., 40.), egui::Button::new("Retry"))
                            .clicked()
                            .then(|| {
                                let mut chat = self.chatgpt.clone();
                                let is_ready = self.is_ready.clone();
                                self.complete_handle.replace(tokio::spawn(async move {
                                    is_ready.store(false, atomic::Ordering::Relaxed);
                                    chat.retry().await.ok();
                                    is_ready.store(true, atomic::Ordering::Relaxed);
                                }));
                            });
                    }
                });
            });
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading(chat.model);
            ui.separator();
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        for message in chat.messages {
                            message_container(
                                ui,
                                |ui| {
                                    let content = message.content.to_string();
                                    ui.add(
                                        egui::Label::new(egui::RichText::new(&content))
                                            .sense(egui::Sense::click()),
                                    )
                                    .clicked()
                                    .then(|| {
                                        ui.output_mut(|o| o.copied_text = content);
                                        self.toasts
                                            .success("Copied")
                                            .set_closable(false)
                                            .set_duration(Some(Duration::from_secs(1)));
                                    });
                                },
                                &message.role,
                            );
                        }
                        if let Some(generate) = &generate {
                            {
                                message_container(
                                    ui,
                                    |ui| {
                                        ui.label(generate);
                                    },
                                    &Role::Assistant,
                                );
                            }
                            ui.ctx().request_repaint();
                        } else if let Some(error_message) = &self.error_message {
                            message_container(
                                ui,
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(error_message)
                                            .color(epaint::Color32::RED),
                                    );
                                    ui.button("Retry")
                                },
                                &Role::Assistant,
                            )
                            .clicked()
                            .then(|| {
                                let mut chat = self.chatgpt.clone();
                                tokio::spawn(async move { chat.retry().await })
                            });
                        } else if !is_ready {
                            message_container(
                                ui,
                                |ui| {
                                    ui.spinner();
                                },
                                &Role::Assistant,
                            );
                        }
                    });
                });
        });
        self.toasts.show(ui.ctx());
    }
}

pub fn message_container<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
    role: &Role,
) -> R {
    ui.horizontal(|ui| {
        ui.with_layout(
            match role {
                Role::User => egui::Layout::right_to_left(egui::Align::Min).with_main_wrap(true),
                Role::Assistant => {
                    egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true)
                }
                Role::System => egui::Layout::centered_and_justified(egui::Direction::TopDown)
                    .with_main_wrap(true),
            },
            |ui| ui.group(|ui| add_contents(ui)).inner,
        )
        .inner
    })
    .inner
}
