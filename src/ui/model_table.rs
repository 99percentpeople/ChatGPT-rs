use std::sync::atomic;

use eframe::egui;

use crate::api::models::ModelsAPI;

use super::ModelType;

pub struct ModelTable {
    pub models: ModelsAPI,
    pub model_type: ModelType,
}

pub enum ResponseEvent {
    SelectModel(String),
    None,
}

impl ModelTable {
    const CHAT_MODELS: [&str; 2] = ["gpt-3.5-turbo", "gpt-3.5-turbo-0301"];
    pub fn new(model_type: ModelType) -> Self {
        Self {
            models: ModelsAPI::new(),
            model_type,
        }
    }
}

impl super::View for ModelTable {
    type Response<'a> = ResponseEvent;

    fn ui(&mut self, ui: &mut egui::Ui) -> Self::Response<'_> {
        let mut event = ResponseEvent::None;
        // let models = block_in_place(|| self.models.models.blocking_read().clone());
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
                        tracing::error!("Failed to get models: {}", e);
                    }
                });
            }
            egui::Grid::new("models").striped(true).show(ui, |ui| {
                ui.label("ID");
                ui.label("Action");
                ui.end_row();
                match self.model_type {
                    ModelType::Chat => {
                        for id in Self::CHAT_MODELS {
                            ui.label(id);
                            if ui.button("Select").clicked() {
                                event = ResponseEvent::SelectModel(id.to_string());
                            }
                            ui.end_row();
                        }
                    }

                    ModelType::Complete => todo!(),
                    ModelType::Insert => todo!(),
                }
            });
            // if let Some(models) = models {
            //     let table = egui_extras::TableBuilder::new(ui)
            //         .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            //         .striped(true)
            //         .column(
            //             egui_extras::Column::auto()
            //                 .at_least(10.0)
            //                 .resizable(true)
            //                 .clip(true),
            //         )
            //         .column(
            //             egui_extras::Column::auto()
            //                 .at_least(10.0)
            //                 .resizable(true)
            //                 .clip(true),
            //         )
            //         .column(
            //             egui_extras::Column::auto()
            //                 .at_least(10.0)
            //                 .resizable(true)
            //                 .clip(true),
            //         )
            //         .column(egui_extras::Column::auto());
            //     table
            //         .header(20., |mut header| {
            //             header.col(|ui| {
            //                 ui.strong("ID");
            //             });
            //             header.col(|ui| {
            //                 ui.strong("Owned By");
            //             });
            //             header.col(|ui| {
            //                 ui.strong("Actions");
            //             });
            //         })
            //         .body(|mut body| {
            //             for model in models.data {
            //                 body.row(20., |mut row| {
            //                     row.col(|ui| {
            //                         ui.label(&model.id);
            //                     });
            //                     row.col(|ui| {
            //                         ui.label(&model.owned_by);
            //                     });
            //                     row.col(|ui| {
            //                         if ui.button("Select").clicked() {
            //                             event = ResponseEvent::SelectModel(model);
            //                         }
            //                     });
            //                 })
            //             }
            //         });
            // }
        });
        event
    }
}
