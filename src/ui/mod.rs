mod chat_list;
mod chat_window;
pub mod logger;
mod model_table;
mod parameter_control;
use eframe::egui;

use self::{chat_list::ChatList, chat_window::ChatWindow, logger::LoggerUi};

pub struct ChatApp {
    window: Option<ChatWindow<'static>>,
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
                if let Some(window) = &mut self.window {
                    window.actions(ui);
                    ui.separator();
                }
                for (view, show) in self.widgets.iter_mut() {
                    ui.selectable_label(*show, view.name()).clicked().then(|| {
                        *show = !*show;
                    });
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
            chat_list::ResponseEvent::SelectChat(chat) => {
                self.window = Some(ChatWindow::new(chat));
            }
            chat_list::ResponseEvent::RemoveChat => {
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
                            self.window =
                                Some(ChatWindow::new(self.chat_list.new_chat(None).unwrap()));
                        });
                });
            });
        }
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
