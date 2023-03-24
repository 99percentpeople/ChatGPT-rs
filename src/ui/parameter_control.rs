use eframe::egui;

use crate::api::{Parameter, ParameterRange, ParameterValue};

pub enum ResponseEvent {
    None,
}

pub struct ParameterControler {
    params: Vec<Box<dyn Parameter>>,
}

impl Default for ParameterControler {
    fn default() -> Self {
        Self { params: vec![] }
    }
}

impl ParameterControler {
    pub fn new(params: Vec<Box<dyn Parameter>>) -> Self {
        Self { params }
    }
}

impl super::View for ParameterControler {
    type Response<'a> = ResponseEvent;
    fn ui(&mut self, ui: &mut egui::Ui) -> Self::Response<'_> {
        let event = ResponseEvent::None;
        egui::Grid::new("grid")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                for param in self.params.iter() {
                    match param.get() {
                        ParameterValue::OptionalInteger(n) => {
                            let mut res = match n {
                                Some(n) => {
                                    if ui.checkbox(&mut true, param.name()).changed() {
                                        param.set(ParameterValue::OptionalInteger(None))
                                    }
                                    n
                                }
                                None => {
                                    let ParameterValue::Integer(d) = param.store() else {
                                        continue;
                                    };
                                    if ui.checkbox(&mut false, param.name()).changed() {
                                        param.set(ParameterValue::OptionalInteger(Some(d)))
                                    }
                                    d
                                }
                            };
                            ui.add_enabled_ui(n.is_some(), |ui| {
                                if let ParameterRange::Integer(st, ed) = param.range() {
                                    if ui.add(egui::Slider::new(&mut res, st..=ed)).changed() {
                                        param.set(ParameterValue::OptionalInteger(Some(res)));
                                    };
                                }
                            });
                            ui.end_row();
                        }
                        ParameterValue::Number(mut n) => {
                            ui.label(param.name());
                            if let ParameterRange::Number(st, ed) = param.range() {
                                if ui.add(egui::Slider::new(&mut n, st..=ed)).changed() {
                                    param.set(ParameterValue::Number(n));
                                };
                            }
                            ui.end_row();
                        }
                        ParameterValue::Integer(mut n) => {
                            ui.label(param.name());
                            if let ParameterRange::Integer(st, ed) = param.range() {
                                if ui.add(egui::Slider::new(&mut n, st..=ed)).changed() {
                                    param.set(ParameterValue::Integer(n));
                                };
                            }
                            ui.end_row();
                        }
                        _ => {}
                    }
                }
                // ui.checkbox(&mut self.max_token_checked, "Max Tokens").changed().then(||{
                //     if self.max_token_checked {
                //         event = ResponseEvent::MaxTokens(Some(self.max_tokens));
                //     }else {
                //         event = ResponseEvent::MaxTokens(None);
                //     }
                // });
                // ui.add_enabled(
                //     self.max_token_checked,
                //     egui::Slider::new(&mut self.max_tokens, 1..=2048),
                // )
                // .changed()
                // .then(|| {
                //     event = ResponseEvent::MaxTokens(Some(self.max_tokens));
                // });
                // ui.end_row();
                // ui.add(doc_link_label("Temperature","temperature", "https://platform.openai.com/docs/api-reference/chat/create#chat/create-temperature"));
                // ui.add(egui::Slider::new(&mut self.temperature, 0.0..=2.0))
                //     .changed()
                //     .then(|| {
                //         event = ResponseEvent::Temperature(self.temperature);
                //     });
                // ui.end_row();
                // ui.add(doc_link_label("Top P", "top_p","https://platform.openai.com/docs/api-reference/chat/create#chat/create-top_p"));

                // ui.add(egui::Slider::new(&mut self.top_p, 0.0..=1.0))
                //     .changed()
                //     .then(|| {
                //         event = ResponseEvent::TopP(self.top_p);
                //     });

                // ui.end_row();
                // ui.add(doc_link_label("Presence Penalty", "presence_penalty","https://platform.openai.com/docs/api-reference/chat/create#chat/create-presence_penalty"));
                // ui.add(egui::Slider::new(&mut self.presence_penalty, -2.0..=2.0))
                //     .changed()
                //     .then(|| {
                //         event = ResponseEvent::PresencePenalty(self.presence_penalty);
                //     });

                // ui.end_row();
                // ui.add(doc_link_label("Frequency Penalty", "frequency_penalty","https://platform.openai.com/docs/api-reference/chat/create#chat/create-frequency_penalty"));
                // ui.add(egui::Slider::new(&mut self.frequency_penalty, -2.0..=2.0))
                //     .changed()
                //     .then(|| {
                //         event = ResponseEvent::FrequencyPenalty(self.frequency_penalty);
                //     });
                // ui.end_row();
            });
        event
    }
}

fn doc_link_label<'a>(title: &'a str, name: &'a str, url: &'a str) -> impl egui::Widget + 'a {
    let label = format!("{}:", title);
    move |ui: &mut egui::Ui| {
        ui.hyperlink_to(label, url).on_hover_ui(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Search openai docs for");
                ui.code(name);
            });
        })
    }
}
