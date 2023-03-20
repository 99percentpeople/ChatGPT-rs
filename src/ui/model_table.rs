use std::sync::atomic;

use eframe::egui;
use tokio::task::block_in_place;

use crate::api::models::ModelsAPI;

pub struct ModelTable {
    pub models: ModelsAPI,
    call_back_fn: Option<Box<dyn FnMut(String)>>,
}

impl Default for ModelTable {
    fn default() -> Self {
        Self {
            models: ModelsAPI::new(),
            call_back_fn: None,
        }
    }
}
impl ModelTable {
    pub fn on_select_model(&mut self, call_back_fn: impl FnMut(String) + 'static) {
        self.call_back_fn.replace(Box::new(call_back_fn));
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let models = block_in_place(|| self.models.models.blocking_read().clone());
        let is_ready = self.models.is_ready.load(atomic::Ordering::Relaxed);
        ui.vertical(|ui| {
            ui.heading("Model");
            ui.separator();
            if ui
                .horizontal(|ui| {
                    let btn = ui.add_enabled(is_ready, egui::Button::new("Get Models"));
                    ui.add_visible(!is_ready, egui::Spinner::new());
                    btn
                })
                .inner
                .clicked()
            {
                let mut models = self.models.clone();
                tokio::spawn(async move {
                    if let Err(e) = models.get_models().await {
                        println!("{:?}", e);
                    }
                });
            }

            ui.vertical(|ui| {
                if let Some(models) = models {
                    let table = egui_extras::TableBuilder::new(ui)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .striped(true)
                        .column(
                            egui_extras::Column::auto()
                                .at_least(10.0)
                                .resizable(true)
                                .clip(true),
                        )
                        .column(
                            egui_extras::Column::auto()
                                .at_least(10.0)
                                .resizable(true)
                                .clip(true),
                        )
                        .column(
                            egui_extras::Column::auto()
                                .at_least(10.0)
                                .resizable(true)
                                .clip(true),
                        )
                        .column(egui_extras::Column::auto());
                    table
                        .header(20., |mut header| {
                            header.col(|ui| {
                                ui.strong("ID");
                            });
                            header.col(|ui| {
                                ui.strong("Owned By");
                            });
                            header.col(|ui| {
                                ui.strong("Actions");
                            });
                        })
                        .body(|mut body| {
                            for model in models.data {
                                body.row(20., |mut row| {
                                    row.col(|ui| {
                                        ui.label(&model.id);
                                    });
                                    row.col(|ui| {
                                        ui.label(model.owned_by);
                                    });
                                    row.col(|ui| {
                                        if let Some(callback) = &mut self.call_back_fn {
                                            if ui.button("Select").clicked() {
                                                callback(model.id);
                                            }
                                        }
                                    });
                                })
                            }
                        });
                }
            });
        });
    }
}
