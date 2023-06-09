use super::{
    easy_mark::{self, MemoizedEasymarkHighlighter},
    model_table::ModelTable,
    parameter_control::ParameterControler,
    ModelType, View, Window,
};
use crate::api::{
    chat::{ChatAPI, Role},
    ParameterControl,
};

use eframe::egui::{self, Modifiers};
use egui_notify::Toasts;
use std::{
    cell::RefCell,
    ops::AddAssign,
    rc::Rc,
    sync::{atomic, Arc},
};
use tokio::task::JoinHandle;

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
    highlighters: Vec<Rc<RefCell<easy_mark::MemoizedEasymarkHighlighter>>>,
    enable_markdown: bool,
    edit_focused: bool,
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
            highlighters: Vec::new(),

            enable_markdown: true,
            edit_focused: false,
        }
    }
}

impl super::Window for ChatWindow {
    fn name(&self) -> &str {
        &self.window_name
    }
    fn show(&mut self, ctx: &egui::Context, _open: &mut bool) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.ui(ui);
        });
    }
}

impl super::TabWindow for ChatWindow {
    fn set_name(&mut self, name: String) {
        self.window_name = name;
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
    fn selectable_text(&mut self, ui: &mut egui::Ui, mut text: &str, idx: &mut usize) {
        if self.enable_markdown {
            let highlighter = self.highlighters.get(*idx).cloned().unwrap_or_else(|| {
                let highlighter = Rc::new(RefCell::new(MemoizedEasymarkHighlighter::default()));
                self.highlighters.push(highlighter.clone());
                highlighter
            });
            let mut layouter = |ui: &egui::Ui, easymark: &str, wrap_width: f32| {
                let mut layout_job = highlighter.borrow_mut().highlight(ui, easymark);
                layout_job.wrap.max_width = wrap_width;
                ui.fonts(|f| f.layout_job(layout_job))
            };
            egui::TextEdit::multiline(&mut text)
                .desired_width(f32::INFINITY)
                .desired_rows(1)
                .layouter(&mut layouter)
                .show(ui)
        } else {
            egui::TextEdit::multiline(&mut text)
                .desired_width(f32::INFINITY)
                .desired_rows(1)
                .show(ui)
        }
        .response
        .context_menu(|ui| {
            ui.button("Copy All").clicked().then(|| {
                ui.output_mut(|o| o.copied_text = text.to_string());
                ui.close_menu();
            });
        });
        idx.add_assign(1);
    }
}

impl super::View for ChatWindow {
    type Response = ();
    fn ui(&mut self, ui: &mut egui::Ui) -> Self::Response {
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

        egui::SidePanel::left(format!("left_{}", self.name())).show_animated_inside(
            ui,
            self.show_model_table,
            |ui| match self.model_table.ui(ui) {
                super::model_table::ResponseEvent::SelectModel(id) => {
                    let mut chatgpt = self.chatgpt.clone();
                    tokio::spawn(async move { chatgpt.set_model(id).await });
                }
                _ => {}
            },
        );

        egui::SidePanel::right(format!("right_{}", self.name())).show_animated_inside(
            ui,
            self.show_parameter_control,
            |ui| {
                self.parameter_control.ui(ui);
            },
        );
        egui::TopBottomPanel::top(format!("top_{}", self.name())).show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(&self.window_name);
                ui.separator();
                ui.heading(chat.model);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.enable_markdown, "Markdown");
                });
            });
        });
        egui::TopBottomPanel::bottom(format!("bottom_{}", self.name())).show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                ui.add_enabled_ui(is_ready, |ui| {
                    if self.edit_focused
                        && ui.input_mut(|i| i.consume_key(Modifiers::NONE, egui::Key::Enter))
                    {
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
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut self.text).desired_width(f32::INFINITY),
                    );
                    self.edit_focused = response.has_focus();
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
                        let mut idx = 0;
                        for msg in chat.messages.iter() {
                            message(
                                ui,
                                |ui| {
                                    self.selectable_text(ui, &msg.content, &mut idx);
                                },
                                &msg.role,
                            );
                        }

                        if let Some(generate) = &generate_text {
                            message(
                                ui,
                                |ui| self.selectable_text(ui, &generate, &mut idx),
                                &Role::Assistant,
                            );

                            ui.ctx().request_repaint();
                        } else if is_error {
                            message(
                                ui,
                                |ui| {
                                    self.selectable_text(ui, &generate_text.unwrap(), &mut idx);
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
                        if idx + 1 < self.highlighters.len() {
                            self.highlighters.pop();
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
