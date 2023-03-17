use std::{
    collections::{BTreeMap, VecDeque},
    sync::RwLock,
};

use eframe::{
    egui::{self, TextFormat},
    epaint::{self, text},
};
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumCount, EnumIter, IntoEnumIterator};
use tracing::metadata;
use tracing_subscriber::{
    registry::{self, LookupSpan},
    Layer,
};

pub static LOG: RwLock<VecDeque<LogOutput>> = RwLock::new(VecDeque::new());

pub struct Logger {
    max_level: metadata::Level,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, EnumIter, EnumCount, Display)]
pub enum Level {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl From<&metadata::Level> for Level {
    fn from(value: &metadata::Level) -> Self {
        match value {
            &metadata::Level::ERROR => Self::Error,
            &metadata::Level::WARN => Self::Warn,
            &metadata::Level::INFO => Self::Info,
            &metadata::Level::DEBUG => Self::Debug,
            &metadata::Level::TRACE => Self::Trace,
        }
    }
}

struct JsonVisitor<'a>(&'a mut BTreeMap<String, serde_json::Value>);

impl<'a> tracing::field::Visit for JsonVisitor<'a> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.0.insert(
            field.name().to_string(),
            serde_json::json!(value.to_string()),
        );
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(
            field.name().to_string(),
            serde_json::json!(format!("{value:#?}")),
        );
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogOutput {
    pub level: Level,
    pub target: String,
    pub name: String,
    pub fields: BTreeMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spans: Option<Vec<LogOutput>>,
}

impl<'a, R: LookupSpan<'a>> From<registry::SpanRef<'a, R>> for LogOutput {
    fn from(span: registry::SpanRef<'a, R>) -> Self {
        let extensions = span.extensions();
        let storage = extensions.get::<CustomFieldStorage>().unwrap();
        let field_data: &BTreeMap<String, serde_json::Value> = &storage.0;
        Self {
            level: Level::from(span.metadata().level()),
            target: span.metadata().target().to_string(),
            name: span.metadata().name().to_string(),
            fields: field_data.clone(),
            spans: None,
        }
    }
}

#[derive(Debug)]
struct CustomFieldStorage(BTreeMap<String, serde_json::Value>);

impl<S> Layer<S> for Logger
where
    S: tracing::Subscriber,
    S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        metadata.level() <= &self.max_level
    }
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // 基于 field 值来构建我们自己的 JSON 对象
        let mut fields = BTreeMap::new();
        let mut visitor = JsonVisitor(&mut fields);
        attrs.record(&mut visitor);

        // 使用之前创建的 newtype 包裹下
        let storage = CustomFieldStorage(fields);

        // 获取内部 span 数据的引用
        let span = ctx.span(id).unwrap();
        // 获取扩展，用于存储我们的 span 数据
        let mut extensions = span.extensions_mut();
        // 存储！
        extensions.insert::<CustomFieldStorage>(storage);
    }
    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // 获取正在记录数据的 span
        let span = ctx.span(id).unwrap();

        // 获取数据的可变引用，该数据是在 on_new_span 中创建的
        let mut extensions_mut = span.extensions_mut();
        let custom_field_storage: &mut CustomFieldStorage =
            extensions_mut.get_mut::<CustomFieldStorage>().unwrap();
        let json_data: &mut BTreeMap<String, serde_json::Value> = &mut custom_field_storage.0;

        // 使用我们的访问器老朋友
        let mut visitor = JsonVisitor(json_data);
        values.record(&mut visitor);
    }
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        // All of the span context
        let spans = ctx
            .event_scope(event)
            .and_then(|scope| Some(scope.map(LogOutput::from).collect()));

        // The fields of the event
        let mut fields = BTreeMap::new();
        let mut visitor = JsonVisitor(&mut fields);
        event.record(&mut visitor);

        let output = LogOutput {
            level: event.metadata().level().into(),
            target: event.metadata().target().to_string(),
            name: event.metadata().name().to_string(),
            fields,
            spans,
        };
        LOG.write().unwrap().push_front(output);
    }
}
impl Logger {
    pub fn new(max_level: metadata::Level) -> Self {
        Self { max_level }
    }
}
pub struct LoggerUi {
    log_levels: [bool; Level::COUNT],
    search_term: String,
    span_filter: String,
    target_filter: String,
    regex: Option<regex::Regex>,
    search_case_sensitive: bool,
    search_use_regex: bool,
    copy_text: String,
    max_log_length: usize,
}

impl Default for LoggerUi {
    fn default() -> Self {
        Self {
            log_levels: [false, true, true, true, true],
            search_term: String::new(),
            span_filter: String::new(),
            target_filter: String::new(),
            search_case_sensitive: false,
            regex: None,
            search_use_regex: false,
            copy_text: String::new(),
            max_log_length: 20,
        }
    }
}

impl LoggerUi {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Search: ");

            let response = ui.text_edit_singleline(&mut self.search_term);
            ui.button("ｘ").clicked().then(|| self.search_term.clear());
            let mut config_changed = false;

            if ui
                .selectable_label(self.search_case_sensitive, "Aa")
                .on_hover_text("Case sensitive")
                .clicked()
            {
                self.search_case_sensitive = !self.search_case_sensitive;
                config_changed = true;
            };
            if ui
                .selectable_label(self.search_use_regex, ".*")
                .on_hover_text("Use regex")
                .clicked()
            {
                self.search_use_regex = !self.search_use_regex;
                config_changed = true;
            }
            if self.search_use_regex && (response.changed() || config_changed) {
                self.regex = RegexBuilder::new(&self.search_term)
                    .case_insensitive(!self.search_case_sensitive)
                    .build()
                    .ok()
            };
        });
        ui.collapsing("Filter", |ui| {
            egui::Grid::new("filter_grid")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Span: ");
                    ui.text_edit_singleline(&mut self.span_filter);
                    ui.button("ｘ").clicked().then(|| self.span_filter.clear());
                    ui.end_row();
                    ui.label("Target: ");
                    ui.text_edit_singleline(&mut self.target_filter);
                    ui.button("ｘ")
                        .clicked()
                        .then(|| self.target_filter.clear());
                    ui.end_row();
                });
        });

        // ui.horizontal(|ui| {
        //     if ui.button("Sort").clicked() {
        //         logs.sort()
        //     }
        // });

        ui.horizontal(|ui| {
            ui.label("Max Log output");
            ui.add(
                egui::widgets::DragValue::new(&mut self.max_log_length)
                    .speed(1)
                    .clamp_range(1..=1000),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                ui.menu_button("Log Levels", |ui| {
                    for level in Level::iter() {
                        if ui
                            .selectable_label(self.log_levels[level as usize], level.to_string())
                            .clicked()
                        {
                            self.log_levels[level as usize] = !self.log_levels[level as usize];
                        }
                    }
                });
                if ui.button("Clear").clicked() {
                    LOG.write().unwrap().clear();
                }
            });
        });
        ui.separator();
        let logs = LOG.read().unwrap();
        let logs_len = logs.len();
        let log_levels = self.log_levels.clone();
        let logs_iter = logs
            .iter()
            .filter(|log| log_levels[log.level as usize])
            .filter(|log| {
                log.spans.as_ref().is_some_and(|span| {
                    span.iter()
                        .find(|span| span.name.contains(&self.span_filter))
                        .is_some()
                })
            })
            .filter(|log| log.target.contains(&self.target_filter))
            .take(self.max_log_length);

        let mut logs_displayed_content = logs_iter.collect::<Vec<_>>();
        logs_displayed_content.reverse();
        let mut logs_displayed: usize = 0;
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .max_height(ui.available_height() - 40.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                logs_displayed_content.iter().for_each(|data| {
                    let content = &serde_json::to_string_pretty(&data).unwrap();
                    if !self.search_term.is_empty() && !self.match_string(content) {
                        return;
                    }
                    let mut job = text::LayoutJob::default();
                    // let first_row_indentation = 10.0;
                    let (level, color) = match data.level {
                        Level::Warn => ("[WARN]", epaint::Color32::YELLOW),
                        Level::Error => ("[ERROR]", epaint::Color32::RED),
                        Level::Info => ("[INFO]", epaint::Color32::LIGHT_BLUE),
                        Level::Debug => ("[DEBUG]", epaint::Color32::LIGHT_GREEN),
                        Level::Trace => ("[TRACE]", epaint::Color32::LIGHT_GRAY),
                    };
                    job.append(
                        &format!("{}\n", level),
                        0.,
                        TextFormat {
                            color,
                            ..Default::default()
                        },
                    );
                    job.append(
                        &content,
                        0.,
                        TextFormat {
                            ..Default::default()
                        },
                    );
                    let l = egui::Label::new(job);
                    // let copy_text = l.text().to_string();
                    ui.add(l);

                    logs_displayed += 1;
                    self.copy_text += &content;
                });
            });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(format!("Displayed: {}", logs_displayed));
            ui.label(format!("Log size: {}", logs_len));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Copy").clicked() {
                    ui.output_mut(|o| o.copied_text = self.copy_text.to_string());
                }
            });
        });

        // has to be cleared after every frame
        self.copy_text.clear();
    }
    fn match_string(&self, string: &str) -> bool {
        if self.search_use_regex {
            if let Some(matcher) = &self.regex {
                matcher.is_match(string)
            } else {
                // Failed to compile
                false
            }
        } else {
            if self.search_case_sensitive {
                string.contains(&self.search_term)
            } else {
                string
                    .to_lowercase()
                    .contains(&self.search_term.to_lowercase())
            }
        }
    }
}
