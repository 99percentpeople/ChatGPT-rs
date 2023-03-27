use crate::api::{
    chat::{ChatAPI, Role},
    ParameterControl,
};
use eframe::egui::{self, Modifiers};
use egui_notify::Toasts;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{atomic, Arc},
};
use tokio::task::JoinHandle;

use super::{
    easy_mark, model_table::ModelTable, parameter_control::ParameterControler, ModelType, View,
};

pub struct ChatWindow {
    window_name: String,
    chatgpt: ChatAPI,
    text: String,
    complete_handle: Option<JoinHandle<()>>,
    is_ready: Arc<atomic::AtomicBool>,
    show_model_table: bool,
    show_parameter_control: bool,
    model_table: ModelTable,
    parameter_control: ParameterControler,
    toasts: Toasts,
    highlighter: Rc<RefCell<easy_mark::MemoizedEasymarkHighlighter>>,
    enable_markdown: bool,
}

impl ChatWindow {
    pub fn new(window_name: String, chatgpt: ChatAPI) -> Self {
        let model_table = ModelTable::new(ModelType::Chat);
        let parameter_control = ParameterControler::new(chatgpt.params());
        Self {
            window_name,
            chatgpt,
            text: String::new(),
            complete_handle: None,
            is_ready: Arc::new(atomic::AtomicBool::new(true)),
            model_table,
            show_model_table: false,
            show_parameter_control: false,
            parameter_control,
            toasts: Toasts::default(),
            highlighter: Rc::new(RefCell::new(
                easy_mark::MemoizedEasymarkHighlighter::default(),
            )),
            enable_markdown: true,
        }
    }
}

impl super::MainWindow for ChatWindow {
    fn name(&self) -> &str {
        &self.window_name
    }
    fn show(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.ui(ui);
        });
    }
    fn actions(&mut self, ui: &mut egui::Ui) {
        ui.selectable_label(self.show_model_table, "Model")
            .clicked()
            .then(|| {
                self.show_model_table = !self.show_model_table;
            });
        ui.selectable_label(self.show_parameter_control, "Tuning")
            .clicked()
            .then(|| {
                self.show_parameter_control = !self.show_parameter_control;
            });
    }
}

impl ChatWindow {
    fn selectable_text(&self, ui: &mut egui::Ui, mut text: &str) {
        if self.enable_markdown {
            let highlighter = self.highlighter.clone();
            let mut layouter = |ui: &egui::Ui, easymark: &str, wrap_width: f32| {
                let mut layout_job = highlighter.borrow_mut().highlight(ui, easymark);
                layout_job.wrap.max_width = wrap_width;
                ui.fonts(|f| f.layout_job(layout_job))
            };
            egui::TextEdit::multiline(&mut text)
                .desired_width(f32::INFINITY)
                .desired_rows(1)
                .layouter(&mut layouter)
                .show(ui);
        } else {
            egui::TextEdit::multiline(&mut text)
                .desired_width(f32::INFINITY)
                .desired_rows(1)
                .show(ui);
        }
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
            .back()
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
                self.parameter_control.ui(ui);
            },
        );
        egui::TopBottomPanel::top("chat_top_panel").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(&self.window_name);
                ui.separator();
                ui.heading(chat.model);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.enable_markdown, "Markdown");
                });
            });
        });
        egui::TopBottomPanel::bottom("bottom_panel").show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                ui.add_enabled_ui(is_ready, |ui| {
                    if ui.input_mut(|i| i.consume_key(Modifiers::NONE, egui::Key::Enter)) {
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
                            return;
                        }
                    }

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
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        for msg in chat.messages {
                            message(
                                ui,
                                |ui| {
                                    let content = msg.content.to_string();
                                    self.selectable_text(ui, &content);
                                },
                                &msg.role,
                            );
                        }
                        if let Some(generate) = &generate_text {
                            {
                                message(
                                    ui,
                                    |ui| {
                                        self.selectable_text(ui, &generate);
                                    },
                                    &Role::Assistant,
                                );
                            }
                            ui.ctx().request_repaint();
                        } else if is_error {
                            message(
                                ui,
                                |ui| {
                                    self.selectable_text(ui, &generate_text.unwrap());
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
                            message(
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

pub fn message<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
    role: &Role,
) -> R {
    ui.group(|ui| {
        ui.vertical(|ui| {
            ui.label(format!("{}: ", role.to_string()));
            add_contents(ui)
        })
        .inner
    })
    .inner
}
