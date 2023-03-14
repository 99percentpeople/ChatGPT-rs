mod model_table;
use std::{sync::atomic, time::Duration};

use crate::api::chat::{ChatGPT, Role};
use eframe::egui;

use self::model_table::ModelTable;
use egui_notify::Toasts;
pub struct ChatApp {
    chat: ChatGPT,
    text: String,
    model_table: model_table::ModelTable,
    toasts: Toasts,
    max_tokens: u32,
    temperature: f32,
    top_p: f32,
    presence_penalty: f32,
    frequency_penalty: f32,
}
impl ChatApp {
    pub fn new_with_chat(cc: &eframe::CreationContext, chat: ChatGPT) -> Self {
        setup_fonts(&cc.egui_ctx);
        let mut model_table = ModelTable::default();
        let chat1 = chat.clone();
        model_table.on_select_model(move |model| {
            let mut chat1 = chat1.clone();
            tokio::spawn(async move {
                chat1.set_model(model).await;
            });
        });
        Self {
            chat,
            text: String::new(),
            toasts: Toasts::default(),
            model_table,
            max_tokens: 2048,
            temperature: 1.,
            top_p: 1.,
            presence_penalty: 0.,
            frequency_penalty: 0.,
        }
    }
}
impl eframe::App for ChatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let (chat, generate) = tokio::task::block_in_place(|| {
            (
                self.chat.chat.blocking_read().clone(),
                self.chat.pending_generate.blocking_read().clone(),
            )
        });

        let is_ready = self.chat.is_ready.load(atomic::Ordering::Relaxed);
        if let Some(max_tokens) = chat.max_tokens {
            self.max_tokens = max_tokens;
        }
        if let Some(temperature) = chat.temperature {
            self.temperature = temperature;
        }
        if let Some(top_p) = chat.top_p {
            self.top_p = top_p;
        }
        if let Some(presence_penalty) = chat.presence_penalty {
            self.presence_penalty = presence_penalty;
        }
        if let Some(frequency_penalty) = chat.frequency_penalty {
            self.frequency_penalty = frequency_penalty;
        }
        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            self.model_table.ui(ui);
        });
        egui::SidePanel::right("right_panel").show(ctx, |ui| {
            egui::Grid::new("grid")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Max Tokens");
                    ui.add(egui::Slider::new(&mut self.max_tokens, 1..=2048));
                    ui.end_row();
                    ui.label("Temperature");
                    ui.add(egui::Slider::new(&mut self.temperature, 0.0..=1.0));
                    ui.end_row();
                    ui.label("Top P");
                    ui.add(egui::Slider::new(&mut self.top_p, 0.0..=1.0));
                    ui.end_row();
                    ui.label("Presence Penalty");
                    ui.add(egui::Slider::new(&mut self.presence_penalty, -2.0..=2.0));
                    ui.end_row();
                    ui.label("Frequency Penalty");
                    ui.add(egui::Slider::new(&mut self.frequency_penalty, -2.0..=2.0));
                    ui.end_row();
                });
        });
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.heading(chat.model);
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                ui.add_enabled_ui(is_ready, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.text).desired_width(f32::INFINITY));
                    ui.add_space(5.);
                    ui.horizontal(|ui| {
                        if ui
                            .add_sized(egui::vec2(50., 40.), egui::Button::new("Send"))
                            .clicked()
                        {
                            let input_text = self.text.trim().to_string();
                            if !input_text.is_empty() {
                                let mut chat = self.chat.clone();
                                tokio::spawn(async move {
                                    chat.is_ready.store(false, atomic::Ordering::Relaxed);
                                    if let Err(e) = chat.question(input_text).await {
                                        println!("Error sending message: {}", e);
                                    }
                                    chat.is_ready.store(true, atomic::Ordering::Relaxed);
                                });
                                self.text.clear();
                            }
                        }
                        if ui
                            .add_sized(egui::vec2(50., 40.), egui::Button::new("Clear"))
                            .clicked()
                        {
                            let mut chat = self.chat.clone();
                            tokio::spawn(async move {
                                chat.clear_message().await;
                            });
                        }
                    });
                });
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.vertical(|ui| {
                    for message in chat.messages {
                        message_container(ui, &message.content, &message.role, &mut self.toasts);
                    }
                    if let Some(generate) = generate.as_ref() {
                        if let Some(content) = generate.content.as_ref() {
                            message_container(ui, &content, &Role::Assistant, &mut self.toasts);
                        } else {
                            ui.spinner();
                        }

                        ctx.request_repaint();
                    }
                });
            });
        });
        self.toasts.show(ctx);
    }
}

fn setup_fonts(ctx: &egui::Context) {
    // Start with the default fonts (we will be adding to them rather than replacing them).
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "msyhl".to_owned(),
        egui::FontData::from_static(include_bytes!("c:\\windows\\fonts\\msyhl.ttc")),
    );
    fonts.font_data.insert(
        "seguiemj".to_owned(),
        egui::FontData::from_static(include_bytes!("c:\\windows\\fonts\\seguiemj.ttf")),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "msyhl".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(1, "seguiemj".to_owned());
    ctx.set_fonts(fonts);
}

pub fn message_container(ui: &mut egui::Ui, content: &str, role: &Role, toasts: &mut Toasts) {
    ui.with_layout(
        egui::Layout::top_down(match role {
            Role::System => egui::Align::Center,
            Role::User => egui::Align::RIGHT,
            Role::Assistant => egui::Align::LEFT,
        })
        .with_main_wrap(true),
        |ui| {
            ui.group(|ui| {
                if ui
                    .add(egui::Label::new(egui::RichText::new(content)).sense(egui::Sense::click()))
                    .clicked()
                {
                    ui.output_mut(|o| o.copied_text = content.to_string());
                    toasts
                        .success("Copied")
                        .set_closable(false)
                        .set_duration(Some(Duration::from_secs(1)));
                }
            });
        },
    );
}
