mod chat_list;
mod chat_window;
mod complete_window;
mod easy_mark;
pub mod logger;
mod model_table;
mod parameter_control;

use self::{chat_list::ChatList, logger::LoggerUi};
use eframe::egui;
use font_kit::{
    family_name::FamilyName,
    properties::{Properties, Weight},
    source::SystemSource,
};
use strum::{Display, EnumIter};

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, Display)]
#[strum(serialize_all = "snake_case")]
pub enum ModelType {
    Chat,
    Complete,
    Insert,
}

pub struct ChatApp {
    window: Option<Box<dyn MainWindow>>,
    chat_list: ChatList,
    widgets: Vec<(Box<dyn Window>, bool)>,
}
impl ChatApp {
    const DEBUG: bool = {
        #[cfg(debug_assertions)]
        {
            true
        }
        #[cfg(not(debug_assertions))]
        {
            false
        }
    };
    pub fn new(cc: &eframe::CreationContext) -> Self {
        setup_fonts(&cc.egui_ctx);
        let mut widgets = Vec::new();
        let mut chat_list = ChatList::default();
        chat_list.load().ok();
        widgets.push((
            Box::new(LoggerUi::default()) as Box<dyn Window>,
            Self::DEBUG,
        ));
        Self {
            window: None,
            chat_list,
            widgets,
        }
    }
}

impl eframe::App for ChatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (view, show) in self.widgets.iter_mut() {
                    ui.selectable_label(*show, view.name()).clicked().then(|| {
                        *show = !*show;
                    });
                }
                ui.separator();
                if let Some(window) = &mut self.window {
                    window.actions(ui);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::global_dark_light_mode_switch(ui);
                    ui.separator();
                });
            });
        });

        self.widgets
            .iter_mut()
            .for_each(|(view, show)| view.show(ctx, show));
        egui::SidePanel::left("left_chat_panel").show(ctx, |ui| match self.chat_list.ui(ui) {
            chat_list::ResponseEvent::Select(chat) => {
                self.window = Some(chat);
            }
            chat_list::ResponseEvent::Remove => {
                self.window = None;
            }
            chat_list::ResponseEvent::None => {}
        });
        if let Some(window) = &mut self.window {
            window.show(ctx);
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Select a chat to start");
                    ui.button("Create Chat")
                        .on_hover_text("Create a new chat")
                        .clicked()
                        .then(|| {
                            self.window = Some(self.chat_list.new_chat(None).unwrap());
                        });
                });
            });
        }
    }
}

fn setup_fonts(ctx: &egui::Context) {
    // Start with the default fonts (we will be adding to them rather than replacing them).
    let mut fonts = egui::FontDefinitions::default();
    let source = SystemSource::new();
    let data = if let Ok(font) = source.select_best_match(
        &[
            FamilyName::Title("微软雅黑".to_owned()),
            FamilyName::SansSerif,
        ],
        Properties::new().weight(Weight::NORMAL),
    ) {
        let font = match font.load() {
            Ok(font) => font,
            Err(err) => {
                tracing::error!("Failed to load font: {}", err);
                return;
            }
        };
        tracing::info!("Using font: {:?}", font);
        let Some(font_data) = font.copy_font_data() else {
            return;
        };
        let data = Box::leak((*font_data).clone().into_boxed_slice());
        data
    } else {
        return;
    };

    fonts
        .font_data
        .insert("system".to_owned(), egui::FontData::from_static(data));
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "system".to_owned());

    let mono = if let Ok(font) = source.select_best_match(
        &[
            FamilyName::Title("YaHei Consolas Hybrid".to_owned()),
            FamilyName::Title("Consolas".to_owned()),
            FamilyName::Monospace,
        ],
        Properties::new().weight(Weight::NORMAL),
    ) {
        let font = match font.load() {
            Ok(font) => font,
            Err(err) => {
                tracing::error!("Failed to load font: {}", err);
                return;
            }
        };
        tracing::info!("Using font: {:?}", font);
        let Some(font_data) = font.copy_font_data() else {
            return;
        };
        let data = Box::leak((*font_data).clone().into_boxed_slice());
        data
    } else {
        return;
    };

    fonts
        .font_data
        .insert("mono".to_owned(), egui::FontData::from_static(mono));

    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "mono".to_owned());
    ctx.set_fonts(fonts);
}

pub trait MainWindow {
    fn name(&self) -> &'static str;
    fn show(&mut self, ctx: &egui::Context);
    fn actions(&mut self, _ui: &mut egui::Ui) {}
}

pub trait Window {
    fn name(&self) -> &'static str;
    fn show(&mut self, ctx: &egui::Context, open: &mut bool);
}

pub trait View {
    type Response<'a>
    where
        Self: 'a;
    fn ui<'a>(&'a mut self, ui: &mut egui::Ui) -> Self::Response<'a>;
}
