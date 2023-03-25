use eframe::egui;

use crate::api::complete::CompleteAPI;

use super::{easy_mark, MainWindow, View};
use poll_promise::Promise;
pub struct CompleteWindow {
    window_name: String,
    complete: CompleteAPI,
    text: String,
    promise: Option<Promise<Result<String, anyhow::Error>>>,
    highlighter: easy_mark::MemoizedEasymarkHighlighter,
    enable_markdown: bool,
}

impl CompleteWindow {
    pub fn new(window_name: String, complete: CompleteAPI) -> Self {
        Self {
            window_name,
            text: tokio::task::block_in_place(|| complete.complete.blocking_read().prompt.clone()),
            complete,
            promise: None,
            highlighter: Default::default(),
            enable_markdown: true,
        }
    }
}

impl MainWindow for CompleteWindow {
    fn name(&self) -> &str {
        &self.window_name
    }

    fn show(&mut self, ctx: &eframe::egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.ui(ui);
        });
    }
}

impl View for CompleteWindow {
    type Response<'a> = ();
    fn ui(&mut self, ui: &mut egui::Ui) -> Self::Response<'_> {
        let generate =
            tokio::task::block_in_place(|| self.complete.pending_generate.blocking_read().clone());
        let is_ready = generate.is_none() && self.promise.is_none();
        if !is_ready {
            ui.ctx().request_repaint();
        }
        if let Some(generate) = generate {
            self.text = generate;
        }
        if let Some(promise) = &self.promise {
            if let Some(text) = promise.ready() {
                if let Ok(text) = text {
                    self.text = text.clone();
                }
                self.promise = None;
            }
        }
        egui::TopBottomPanel::top("complete_top").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(&self.window_name);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.enable_markdown, "Markdown");
                });
            });
        });
        egui::TopBottomPanel::bottom("complete_bottom").show_inside(ui, |ui| {
            ui.add_space(5.);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(is_ready, |ui| {
                    ui.add_sized([50., 40.], egui::Button::new("Complete"))
                        .clicked()
                        .then(|| {
                            let mut complete = self.complete.clone();
                            self.promise = Some(Promise::spawn_async(async move {
                                match complete.generate().await {
                                    Ok(res) => Ok(res),
                                    Err(e) => {
                                        tracing::error!("{}", e);
                                        Err(e)
                                    }
                                }
                            }));
                        });
                });
                if !is_ready {
                    ui.add_sized([50., 40.], egui::Button::new("Abort"))
                        .clicked()
                        .then(|| {
                            if let Some(promise) = self.promise.take() {
                                promise.abort();
                                let mut complete = self.complete.clone();
                                tokio::spawn(async move {
                                    let pending_generate =
                                        complete.pending_generate.write().await.take();
                                    if let Some(text) = pending_generate {
                                        complete.set_prompt(text).await;
                                    }
                                });
                            }
                        });
                }
            });
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.add_enabled_ui(is_ready, |ui| {
                        if self.enable_markdown {
                            let mut layouter = |ui: &egui::Ui, easymark: &str, wrap_width: f32| {
                                let mut layout_job = self.highlighter.highlight(ui, easymark);
                                layout_job.wrap.max_width = wrap_width;
                                ui.fonts(|f| f.layout_job(layout_job))
                            };

                            ui.add_sized(
                                ui.available_size(),
                                egui::TextEdit::multiline(&mut self.text)
                                    .desired_width(f32::INFINITY)
                                    .layouter(&mut layouter),
                            )
                        } else {
                            ui.add_sized(
                                ui.available_size(),
                                egui::TextEdit::multiline(&mut self.text)
                                    .desired_width(f32::INFINITY),
                            )
                        }
                        .changed()
                        .then(|| {
                            let mut complete = self.complete.clone();
                            let text = self.text.clone();
                            tokio::spawn(async move {
                                complete.set_prompt(text).await;
                            });
                        });
                    });
                });
        });
    }
}
