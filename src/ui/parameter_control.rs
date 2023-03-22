use eframe::egui;

pub enum ResponseEvent {
    MaxTokens(Option<u32>),
    Temperature(f32),
    TopP(f32),
    PresencePenalty(f32),
    FrequencyPenalty(f32),
    None,
}

pub struct ParameterControl {
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub presence_penalty: f32,
    pub frequency_penalty: f32,

    max_token_checked: bool,
}

impl ParameterControl {
    pub fn set_max_tokens(&mut self, max_tokens: u32) {
        self.max_tokens = max_tokens;
    }
    pub fn set_temperature(&mut self, temperature: f32) {
        self.temperature = temperature;
    }
    pub fn set_top_p(&mut self, top_p: f32) {
        self.top_p = top_p;
    }
    pub fn set_presence_penalty(&mut self, presence_penalty: f32) {
        self.presence_penalty = presence_penalty;
    }
    pub fn set_frequency_penalty(&mut self, frequency_penalty: f32) {
        self.frequency_penalty = frequency_penalty;
    }
    pub fn set_max_token_checked(&mut self, max_token_checked: bool) {
        self.max_token_checked = max_token_checked;
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
        }
    }
}

impl super::View for ParameterControl {
    type Response<'a> = ResponseEvent;
    fn ui(&mut self, ui: &mut egui::Ui) -> Self::Response<'_> {
        let mut event = ResponseEvent::None;
        egui::Grid::new("grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.checkbox(&mut self.max_token_checked, "Max Tokens").changed().then(||{
                    if self.max_token_checked {
                        event = ResponseEvent::MaxTokens(Some(self.max_tokens));
                    }else {
                        event = ResponseEvent::MaxTokens(None);
                    }
                });
                ui.add_enabled(
                    self.max_token_checked,
                    egui::Slider::new(&mut self.max_tokens, 1..=2048),
                )
                .changed()
                .then(|| {
                    event = ResponseEvent::MaxTokens(Some(self.max_tokens));
                });
                ui.end_row();
                ui.add(doc_link_label("Temperature","temperature", "https://platform.openai.com/docs/api-reference/chat/create#chat/create-temperature"));
                ui.add(egui::Slider::new(&mut self.temperature, 0.0..=2.0))
                    .changed()
                    .then(|| {
                        event = ResponseEvent::Temperature(self.temperature);
                    });
                ui.end_row();
                ui.add(doc_link_label("Top P", "top_p","https://platform.openai.com/docs/api-reference/chat/create#chat/create-top_p"));

                ui.add(egui::Slider::new(&mut self.top_p, 0.0..=1.0))
                    .changed()
                    .then(|| {
                        event = ResponseEvent::TopP(self.top_p);
                    });

                ui.end_row();
                ui.add(doc_link_label("Presence Penalty", "presence_penalty","https://platform.openai.com/docs/api-reference/chat/create#chat/create-presence_penalty"));
                ui.add(egui::Slider::new(&mut self.presence_penalty, -2.0..=2.0))
                    .changed()
                    .then(|| {
                        event = ResponseEvent::PresencePenalty(self.presence_penalty);
                    });

                ui.end_row();
                ui.add(doc_link_label("Frequency Penalty", "frequency_penalty","https://platform.openai.com/docs/api-reference/chat/create#chat/create-frequency_penalty"));
                ui.add(egui::Slider::new(&mut self.frequency_penalty, -2.0..=2.0))
                    .changed()
                    .then(|| {
                        event = ResponseEvent::FrequencyPenalty(self.frequency_penalty);
                    });
                ui.end_row();
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
