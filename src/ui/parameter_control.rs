use eframe::egui;

pub struct ParameterControl {
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub presence_penalty: f32,
    pub frequency_penalty: f32,

    max_token_checked: bool,
    on_max_tokens_changed: Option<Box<dyn FnMut(Option<u32>)>>,
    on_temperature_changed: Option<Box<dyn FnMut(f32)>>,
    on_top_p_changed: Option<Box<dyn FnMut(f32)>>,
    on_presence_penalty_changed: Option<Box<dyn FnMut(f32)>>,
    on_frequency_penalty_changed: Option<Box<dyn FnMut(f32)>>,
}

impl ParameterControl {
    pub fn on_max_tokens_changed(&mut self, call_back_fn: impl FnMut(Option<u32>) + 'static) {
        self.on_max_tokens_changed.replace(Box::new(call_back_fn));
    }
    pub fn on_temperature_changed(&mut self, call_back_fn: impl FnMut(f32) + 'static) {
        self.on_temperature_changed.replace(Box::new(call_back_fn));
    }
    pub fn on_top_p_changed(&mut self, call_back_fn: impl FnMut(f32) + 'static) {
        self.on_top_p_changed.replace(Box::new(call_back_fn));
    }
    pub fn on_presence_penalty_changed(&mut self, call_back_fn: impl FnMut(f32) + 'static) {
        self.on_presence_penalty_changed
            .replace(Box::new(call_back_fn));
    }
    pub fn on_frequency_penalty_changed(&mut self, call_back_fn: impl FnMut(f32) + 'static) {
        self.on_frequency_penalty_changed
            .replace(Box::new(call_back_fn));
    }
}

impl Default for ParameterControl {
    fn default() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 1.,
            top_p: 1.,
            presence_penalty: 0.,
            frequency_penalty: 0.,

            max_token_checked: false,

            on_max_tokens_changed: None,
            on_temperature_changed: None,
            on_top_p_changed: None,
            on_presence_penalty_changed: None,
            on_frequency_penalty_changed: None,
        }
    }
}

impl ParameterControl {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.checkbox(&mut self.max_token_checked, "Max Tokens").changed().then(||{
                    let Some(call_back_fn) = self.on_max_tokens_changed.as_mut() else { return };
                    if self.max_token_checked {
                        call_back_fn(Some(self.max_tokens));
                    } else {
                        call_back_fn(None);
                    }
                });
                ui.add_enabled(
                    self.max_token_checked,
                    egui::Slider::new(&mut self.max_tokens, 1..=2048),
                )
                .changed()
                .then(|| {
                    let Some(call_back_fn) = self.on_max_tokens_changed.as_mut() else { return };
                    if self.max_token_checked {
                        call_back_fn(Some(self.max_tokens));
                    } else {
                        call_back_fn(None);
                    }
                });
                ui.end_row();
                ui.add(doc_link_label("Temperature","temperature", "https://platform.openai.com/docs/api-reference/chat/create#chat/create-temperature"));
                ui.add(egui::Slider::new(&mut self.temperature, 0.0..=2.0))
                    .changed()
                    .then(|| {
                        if let Some(call_back_fn) = &mut self.on_temperature_changed {
                            call_back_fn(self.temperature);
                        }
                    });
                ui.end_row();
                ui.add(doc_link_label("Top P", "top_p","https://platform.openai.com/docs/api-reference/chat/create#chat/create-top_p"));

                ui.add(egui::Slider::new(&mut self.top_p, 0.0..=1.0))
                    .changed()
                    .then(|| {
                        if let Some(call_back_fn) = &mut self.on_top_p_changed {
                            call_back_fn(self.top_p);
                        }
                    });

                ui.end_row();
                ui.add(doc_link_label("Presence Penalty", "presence_penalty","https://platform.openai.com/docs/api-reference/chat/create#chat/create-presence_penalty"));
                ui.add(egui::Slider::new(&mut self.presence_penalty, -2.0..=2.0))
                    .changed()
                    .then(|| {
                        if let Some(call_back_fn) = &mut self.on_presence_penalty_changed {
                            call_back_fn(self.presence_penalty);
                        }
                    });

                ui.end_row();
                ui.add(doc_link_label("Frequency Penalty", "frequency_penalty","https://platform.openai.com/docs/api-reference/chat/create#chat/create-frequency_penalty"));
                ui.add(egui::Slider::new(&mut self.frequency_penalty, -2.0..=2.0))
                    .changed()
                    .then(|| {
                        if let Some(call_back_fn) = &mut self.on_frequency_penalty_changed {
                            call_back_fn(self.frequency_penalty);
                        }
                    });
                ui.end_row();
            });
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

// fn doc_link_label_with_checkbox<'a>(
//     checked: &'a mut bool,
//     title: &'a str,
//     name: &'a str,
//     url: &'a str,
// ) -> impl egui::Widget + 'a {
//     let label = format!("{}:", title);
//     move |ui: &mut egui::Ui| {
//         ui.checkbox(checked, egui::RichText::new(title))
//             .on_hover_ui(|ui| {
//                 ui.horizontal_wrapped(|ui| {
//                     ui.label("Search egui docs for");
//                     ui.code(name);
//                 });
//             })
//     }
// }
