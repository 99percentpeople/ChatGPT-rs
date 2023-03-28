mod chat_window;
mod complete_window;
mod components;
mod easy_mark;
mod list_view;
pub mod logger;
mod model_table;
mod parameter_control;

use self::{list_view::ListView, logger::LoggerUi};
use eframe::{
    egui::{self, TextStyle},
    epaint::{FontFamily, FontId},
};

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
    Edit,
}

pub struct ChatApp {
    list_view: ListView,
    widgets: Vec<(Box<dyn Window<Response = ()>>, bool)>,
    tree: egui_dock::Tree<String>,
    // buffers: BTreeMap<String, &dyn MainWindow>,
    expand_list: bool,
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
        let mut list_view = ListView::default();

        list_view.load("./chats.json").ok();
        widgets.push((
            Box::new(LoggerUi::default()) as Box<dyn Window<Response = ()>>,
            Self::DEBUG,
        ));
        Self {
            list_view,
            widgets,
            expand_list: true,
            // buffers: BTreeMap::new(),
            tree: egui_dock::Tree::default(),
        }
    }
}

impl eframe::App for ChatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    ui.button("Load").clicked().then(|| {
                        if let Err(e) = self.list_view.load("./chats.json") {
                            tracing::error!("{}", e);
                        }
                        ui.close_menu();
                    });
                    ui.button("Save").clicked().then(|| {
                        if let Err(e) = self.list_view.save("./chats.json") {
                            tracing::error!("{}", e);
                        }
                        ui.close_menu();
                    });
                });
                if ui.selectable_label(self.expand_list, "List").clicked() {
                    self.expand_list = !self.expand_list;
                };

                ui.separator();

                if let Some((_, tab)) = self.tree.find_active_focused() {
                    self.list_view.action(tab, ui);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::global_dark_light_mode_switch(ui);
                    ui.separator();
                    for (view, show) in self.widgets.iter_mut() {
                        ui.selectable_label(*show, view.name()).clicked().then(|| {
                            *show = !*show;
                        });
                    }
                });
            });
        });

        self.widgets
            .iter_mut()
            .for_each(|(view, show)| view.show(ctx, show));

        egui::SidePanel::left("left_chat_panel").show_animated(ctx, self.expand_list, |ui| {
            match self.list_view.ui(ui) {
                list_view::ResponseEvent::Select(label) => {
                    if let Some(index) = self.tree.find_tab(&label) {
                        self.tree.set_focused_node(index.0)
                    } else {
                        self.tree.push_to_focused_leaf(label)
                    }
                }
                list_view::ResponseEvent::Remove(label) => {
                    if let Some(index) = self.tree.find_tab(&label) {
                        self.tree.remove_tab(index);
                    }
                }
                list_view::ResponseEvent::Rename(from, to) => {
                    if let Some(index) = self.tree.find_tab(&from) {
                        self.tree.remove_tab(index);
                        self.tree.remove_empty_leaf();
                        self.tree.push_to_first_leaf(to.clone());
                    }
                }
                list_view::ResponseEvent::None => {}
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut style = egui_dock::Style::from_egui(&ui.style());
            style.tab_include_scrollarea = false;
            egui_dock::DockArea::new(&mut self.tree)
                .style(style)
                .show_inside(ui, &mut self.list_view);
        });
    }
}

pub trait TabWindow: View {
    fn set_name(&mut self, name: String);
    fn name(&self) -> &str;
    fn show(&mut self, ctx: &egui::Context);
    fn actions(&mut self, _ui: &mut egui::Ui) {}
}

pub trait Window: View {
    fn name(&self) -> &str;
    fn show(&mut self, ctx: &egui::Context, open: &mut bool);
}

pub trait View {
    type Response;
    fn ui(&mut self, ui: &mut egui::Ui) -> Self::Response;
}

fn setup_fonts(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        TextStyle::Name("Heading1".into()),
        FontId::new(36.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Name("Heading2".into()),
        FontId::new(24.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Name("Heading3".into()),
        FontId::new(21.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Name("Heading4".into()),
        FontId::new(18.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Name("Heading5".into()),
        FontId::new(16.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Name("Heading6".into()),
        FontId::new(14.0, FontFamily::Proportional),
    );
    ctx.set_style(style);

    let mut fonts = egui::FontDefinitions::default();
    let source = SystemSource::new();
    let prop = if let Ok(font) = source.select_best_match(
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
        .insert("prop".to_owned(), egui::FontData::from_static(prop));
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "prop".to_owned());

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
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "mono".to_owned());
    ctx.set_fonts(fonts);
}
