use crate::api::chat::{ChatAPI, Role};
use eframe::{egui, epaint};
use egui_notify::Toasts;
use std::{
    sync::{atomic, Arc},
    time::Duration,
};
use tokio::task::JoinHandle;

use super::{model_table::ModelTable, parameter_control::ParameterControl, ModelType, View};

pub struct ChatWindow {
    chatgpt: ChatAPI,
    text: String,
    complete_handle: Option<JoinHandle<()>>,
    is_ready: Arc<atomic::AtomicBool>,
    show_model_table: bool,
    show_parameter_control: bool,
    model_table: ModelTable,
    parameter_control: ParameterControl,
    toasts: Toasts,
}

impl ChatWindow {
    const LINEBREAK_SHORTCUT: egui::KeyboardShortcut =
        egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Enter);
    pub fn new(chatgpt: ChatAPI) -> Self {
        let model_table = ModelTable::new(ModelType::Chat);
        let param = tokio::task::block_in_place(|| chatgpt.data.blocking_read().clone());

        let mut parameter_control = ParameterControl::default();
        parameter_control.set_max_token_checked(param.max_tokens.is_some());
        parameter_control.set_max_tokens(param.max_tokens.unwrap_or(2048));
        parameter_control.set_temperature(param.temperature.unwrap_or(1.));
        parameter_control.set_frequency_penalty(param.frequency_penalty.unwrap_or(0.));
        parameter_control.set_presence_penalty(param.presence_penalty.unwrap_or(0.));
        parameter_control.set_top_p(param.top_p.unwrap_or(1.));
        Self {
            chatgpt,
            text: String::new(),
            complete_handle: None,
            is_ready: Arc::new(atomic::AtomicBool::new(true)),
            model_table,
            show_model_table: false,
            show_parameter_control: false,
            parameter_control,
            toasts: Toasts::default(),
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
    type Response<'a> = ();
    fn ui(&mut self, ui: &mut egui::Ui) -> Self::Response<'_> {
        let chat = tokio::task::block_in_place(|| self.chatgpt.data.blocking_read().clone());
        let generate_res = self.chatgpt.get_generate();
        let is_error = generate_res
            .as_ref()
            .is_some_and(|generate| generate.is_err());
        let generate_text = generate_res.map(|generate| generate.unwrap_or_else(|e| e));

        let is_ready = self.is_ready.load(atomic::Ordering::Relaxed);
        let ready_to_retry = chat
            .messages
            .last()
            .is_some_and(|msg| msg.role == Role::User)
            && is_ready;
        let can_remove_last = !chat.messages.is_empty();
        if is_ready {
            self.complete_handle.take();
        }
        egui::SidePanel::left("left_panel").show_animated_inside(ui, self.show_model_table, |ui| {
            match self.model_table.ui(ui) {
                super::model_table::ResponseEvent::SelectModel(id) => {
                    let mut chatgpt = self.chatgpt.clone();
                    tokio::spawn(async move { chatgpt.set_model(id).await });
                }
                _ => {}
            }
        });

        egui::SidePanel::right("right_panel").show_animated_inside(
            ui,
            self.show_parameter_control,
            |ui| {
                let mut chatgpt = self.chatgpt.clone();
                match self.parameter_control.ui(ui) {
                    super::parameter_control::ResponseEvent::MaxTokens(max_tokens) => {
                        tokio::spawn(async move { chatgpt.set_max_tokens(max_tokens).await });
                    }
                    super::parameter_control::ResponseEvent::Temperature(temperature) => {
                        tokio::spawn(async move { chatgpt.set_temperature(temperature).await });
                    }
                    super::parameter_control::ResponseEvent::TopP(top_p) => {
                        tokio::spawn(async move { chatgpt.set_top_p(top_p).await });
                    }
                    super::parameter_control::ResponseEvent::PresencePenalty(presence_penalty) => {
                        tokio::spawn(async move {
                            chatgpt.set_presence_penalty(presence_penalty).await
                        });
                    }
                    super::parameter_control::ResponseEvent::FrequencyPenalty(
                        frequency_penalty,
                    ) => {
                        tokio::spawn(async move {
                            chatgpt.set_frequency_penalty(frequency_penalty).await
                        });
                    }
                    super::parameter_control::ResponseEvent::None => {}
                }
            },
        );

        egui::TopBottomPanel::bottom("bottom_panel").show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                ui.add_enabled_ui(is_ready, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.text).desired_width(f32::INFINITY));
                    if ui.input_mut(|i| i.consume_shortcut(&Self::LINEBREAK_SHORTCUT)) {
                        // self.text.push_str("\n");
                        return;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
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
                    }
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
                        ui.add_enabled_ui(can_remove_last, |ui| {
                            ui.add_sized(egui::vec2(50., 40.), egui::Button::new("Remove Last"))
                                .clicked()
                                .then(|| {
                                    let mut chat = self.chatgpt.clone();
                                    tokio::spawn(async move {
                                        chat.remove_last().await;
                                    });
                                });
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
                                    chat.generate().await.ok();
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
                        if let Some(generate) = &generate_text {
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
                        } else if is_error {
                            message_container(
                                ui,
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(generate_text.unwrap())
                                            .color(epaint::Color32::RED),
                                    );
                                    ui.button("Retry")
                                },
                                &Role::Assistant,
                            )
                            .clicked()
                            .then(|| {
                                let mut chat = self.chatgpt.clone();
                                tokio::spawn(async move { chat.generate().await })
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