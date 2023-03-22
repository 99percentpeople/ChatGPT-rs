use eframe::egui;

use crate::api::complete::CompleteAPI;

use super::{MainWindow, View};
use poll_promise::Promise;
pub struct CompleteWindow {
    complete: CompleteAPI,
    text: String,
    promise: Option<Promise<Result<String, anyhow::Error>>>,
}

impl CompleteWindow {
    pub fn new(complete: CompleteAPI) -> Self {
        Self {
            text: tokio::task::block_in_place(|| complete.complete.blocking_read().prompt.clone()),
            complete,
            promise: None,
        }
    }
}

impl MainWindow for CompleteWindow {
    fn name(&self) -> &'static str {
        "Complete"
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
        let is_ready = self.promise.is_none();
        if let Some(generate) = generate {
            self.text = generate;
            ui.ctx().request_repaint();
        }
        if let Some(promise) = &self.promise {
            if let Some(text) = promise.ready() {
                if let Ok(text) = text {
                    self.text = text.clone();
                }
                self.promise = None;
            }
        }
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
                            }
                        });
                }
            });
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    egui::TextEdit::multiline(&mut self.text)
                        .desired_width(f32::INFINITY)
                        .show(ui)
                        .response
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
    }
}
