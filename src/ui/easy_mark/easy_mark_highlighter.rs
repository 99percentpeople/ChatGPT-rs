use super::easy_mark_parser;
use super::syntax_highlighting;
use eframe::egui;

/// Highlight easymark, memoizing previous output to save CPU.
///
/// In practice, the highlighter is fast enough not to need any caching.
#[derive(Default)]
pub struct MemoizedEasymarkHighlighter {
    style: egui::Style,
    code: String,
    output: egui::text::LayoutJob,
}

impl MemoizedEasymarkHighlighter {
    pub fn highlight(&mut self, ui: &egui::Ui, code: &str) -> egui::text::LayoutJob {
        let egui_style: &egui::Style = ui.style();
        if (&self.style, self.code.as_str()) != (egui_style, code) {
            self.style = egui_style.clone();
            self.code = code.to_owned();
            self.output = highlight_easymark(ui.ctx(), egui_style, code);
        }
        self.output.clone()
    }
}

pub fn highlight_easymark(
    ctx: &egui::Context,
    egui_style: &egui::Style,
    mut text: &str,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let mut style = easy_mark_parser::Style::default();
    let mut start_of_line = true;
    let theme = syntax_highlighting::CodeTheme::from_style(&egui_style);

    while !text.is_empty() {
        if start_of_line && text.starts_with("```") {
            let language = text[3..]
                .find('\n')
                .map_or_else(|| "text", |i| &text[3..3 + i]);

            let end = text.find("\n```").map_or_else(|| text.len(), |i| i + 4);

            let mut code_job = syntax_highlighting::highlight(ctx, &theme, &text[..end], language);
            let offset = job.text.len();
            code_job.sections.iter_mut().for_each(|s| {
                s.byte_range = (s.byte_range.start + offset)..(s.byte_range.end + offset);
                s.format.background = egui_style.visuals.extreme_bg_color;
            });

            job.sections.append(&mut code_job.sections);
            job.text.push_str(&text[..end]);

            text = &text[end..];
            style = Default::default();
            continue;
        }

        if text.starts_with('`') {
            style.code = true;
            let end = text[1..]
                .find(&['`', '\n'][..])
                .map_or_else(|| text.len(), |i| i + 2);
            job.append(&text[..end], 0.0, format_from_style(egui_style, &style));
            text = &text[end..];
            style.code = false;
            continue;
        }

        let mut skip;

        if text.starts_with('\\') && text.len() >= 2 {
            skip = 2;
        }
        // else if start_of_line && text.starts_with(' ') {
        //     // we don't preview indentation, because it is confusing
        //     skip = 1;
        // } else if start_of_line && text.starts_with("# ") {
        //     style.heading = true;
        //     skip = 2;
        // } else if start_of_line && text.starts_with("> ") {
        //     style.quoted = true;
        //     skip = 2;
        //     // we don't preview indentation, because it is confusing
        // } else if start_of_line && text.starts_with("- ") {
        //     skip = 2;
        //     // we don't preview indentation, because it is confusing
        // }
        else if text.starts_with("**") {
            skip = 2;
            if style.strong {
                // Include the character that is ending this style:
                job.append(&text[..skip], 0.0, format_from_style(egui_style, &style));
                text = &text[skip..];
                skip = 0;
            }
            style.strong ^= true;
        } else if text.starts_with('$') {
            skip = 1;
            if style.small {
                // Include the character that is ending this style:
                job.append(&text[..skip], 0.0, format_from_style(egui_style, &style));
                text = &text[skip..];
                skip = 0;
            }
            style.small ^= true;
        } else if text.starts_with('^') {
            skip = 1;
            if style.raised {
                // Include the character that is ending this style:
                job.append(&text[..skip], 0.0, format_from_style(egui_style, &style));
                text = &text[skip..];
                skip = 0;
            }
            style.raised ^= true;
        } else if text.starts_with('*') {
            skip = 1;
            if style.italics {
                // Include the character that is ending this style:
                job.append(&text[..skip], 0.0, format_from_style(egui_style, &style));
                text = &text[skip..];
                skip = 0;
            }
            style.italics ^= true;
        } else {
            skip = 0;
        }
        // Note: we don't preview underline, strikethrough and italics because it confuses things.

        // Swallow everything up to the next special character:
        let line_end = text[skip..]
            .find('\n')
            .map_or_else(|| text.len(), |i| (skip + i + 1));
        // let end = text[skip..]
        //     .find(&['*', '`', '~', '_', '/', '$', '^', '\\', '<', '['][..])
        //     .map_or_else(|| text.len(), |i| (skip + i).max(1));
        // // Swallow everything up to the next special character:
        let special = ["**", "*", "`", "~", "_", "/", "$", "^", "\\", "<", "["];
        let end = special
            .iter()
            .filter_map(|s| text.find(s))
            .min()
            .map_or_else(|| text.len(), |i| (skip + i).max(1));

        if line_end <= end {
            job.append(
                &text[..line_end],
                0.0,
                format_from_style(egui_style, &style),
            );
            text = &text[line_end..];
            start_of_line = true;
            style = Default::default();
        } else {
            job.append(&text[..end], 0.0, format_from_style(egui_style, &style));
            text = &text[end..];
            start_of_line = false;
        }
    }

    job
}

fn format_from_style(
    egui_style: &egui::Style,
    emark_style: &easy_mark_parser::Style,
) -> egui::text::TextFormat {
    use egui::{Align, Color32, Stroke, TextStyle};

    let color = if emark_style.strong || emark_style.heading {
        egui_style.visuals.strong_text_color()
    } else if emark_style.quoted {
        egui_style.visuals.weak_text_color()
    } else {
        egui_style.visuals.text_color()
    };

    let text_style = if emark_style.heading {
        TextStyle::Heading
    } else if emark_style.code {
        TextStyle::Monospace
    } else if emark_style.small | emark_style.raised {
        TextStyle::Small
    } else {
        TextStyle::Body
    };

    let background = if emark_style.code {
        egui_style.visuals.code_bg_color
    } else {
        Color32::TRANSPARENT
    };

    let underline = if emark_style.underline {
        Stroke::new(1.0, color)
    } else {
        Stroke::NONE
    };

    let strikethrough = if emark_style.strikethrough {
        Stroke::new(1.0, color)
    } else {
        Stroke::NONE
    };

    let valign = if emark_style.raised {
        Align::TOP
    } else {
        Align::BOTTOM
    };

    let italics = if emark_style.italics { true } else { false };

    egui::text::TextFormat {
        font_id: text_style.resolve(egui_style),
        color,
        background,
        italics,
        underline,
        strikethrough,
        valign,
    }
}
