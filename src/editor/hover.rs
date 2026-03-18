/// A parsed section of a hover markdown document.
pub(super) enum HoverSection {
    /// A fenced code block (``` ... ```).
    CodeBlock { lang: String, code: String },
    /// A prose paragraph (may contain inline `**bold**`, `*italic*`, `` `code` ``).
    Text(String),
    /// A horizontal rule (`---`).
    Separator,
}

/// Split raw LSP markdown hover text into typed sections.
pub(super) fn parse_hover_sections(text: &str) -> Vec<HoverSection> {
    let mut sections: Vec<HoverSection> = Vec::new();
    let mut in_code = false;
    let mut code_lang = String::new();
    let mut code_lines: Vec<&str> = Vec::new();
    let mut text_lines: Vec<&str> = Vec::new();

    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            if in_code {
                let t = text_lines.join("\n");
                if !t.trim().is_empty() {
                    sections.push(HoverSection::Text(t));
                }
                text_lines.clear();
                sections.push(HoverSection::CodeBlock {
                    lang: code_lang.clone(),
                    code: code_lines.join("\n"),
                });
                code_lines.clear();
                in_code = false;
            } else {
                let t = text_lines.join("\n");
                if !t.trim().is_empty() {
                    sections.push(HoverSection::Text(t));
                }
                text_lines.clear();
                code_lang = line.trim_start_matches('`').trim().to_string();
                in_code = true;
            }
        } else if in_code {
            code_lines.push(line);
        } else if line.trim() == "---" || line.trim() == "___" || line.trim() == "***" {
            let t = text_lines.join("\n");
            if !t.trim().is_empty() {
                sections.push(HoverSection::Text(t));
            }
            text_lines.clear();
            sections.push(HoverSection::Separator);
        } else {
            text_lines.push(line);
        }
    }
    if in_code && !code_lines.is_empty() {
        sections.push(HoverSection::CodeBlock {
            lang: code_lang,
            code: code_lines.join("\n"),
        });
    } else {
        let t = text_lines.join("\n");
        if !t.trim().is_empty() {
            sections.push(HoverSection::Text(t));
        }
    }
    sections
}

/// Build a LayoutJob for a line of prose, interpreting inline markdown:
/// `**bold**`, `*italic*`, `` `code` ``.
pub(super) fn inline_markdown_job(text: &str, font_size: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob {
        wrap: egui::text::TextWrapping {
            max_width: 500.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let normal_color = egui::Color32::from_rgb(204, 204, 204);
    let bold_color = egui::Color32::WHITE;
    let code_color = egui::Color32::from_rgb(206, 145, 120);
    let code_bg = egui::Color32::from_rgb(40, 40, 40);

    let prop = |sz: f32| egui::FontId::proportional(sz);
    let mono = |sz: f32| egui::FontId::monospace(sz);

    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut current = String::new();

    let flush_normal = |job: &mut egui::text::LayoutJob, s: &mut String| {
        if s.is_empty() { return; }
        job.append(s, 0.0, egui::TextFormat {
            font_id: prop(font_size),
            color: normal_color,
            ..Default::default()
        });
        s.clear();
    };

    while i < chars.len() {
        if i + 1 < chars.len()
            && ((chars[i] == '*' && chars[i + 1] == '*')
                || (chars[i] == '_' && chars[i + 1] == '_'))
        {
            let marker = chars[i];
            flush_normal(&mut job, &mut current);
            i += 2;
            let mut bold = String::new();
            while i + 1 < chars.len() && !(chars[i] == marker && chars[i + 1] == marker) {
                bold.push(chars[i]);
                i += 1;
            }
            if i + 1 < chars.len() { i += 2; }
            if !bold.is_empty() {
                job.append(&bold, 0.0, egui::TextFormat {
                    font_id: prop(font_size),
                    color: bold_color,
                    ..Default::default()
                });
            }
        } else if (chars[i] == '*' || chars[i] == '_')
            && (i + 1 >= chars.len() || chars[i + 1] != chars[i])
        {
            let marker = chars[i];
            flush_normal(&mut job, &mut current);
            i += 1;
            let mut italic = String::new();
            while i < chars.len() && chars[i] != marker {
                italic.push(chars[i]);
                i += 1;
            }
            if i < chars.len() { i += 1; }
            if !italic.is_empty() {
                job.append(&italic, 0.0, egui::TextFormat {
                    font_id: prop(font_size),
                    color: normal_color,
                    italics: true,
                    ..Default::default()
                });
            }
        } else if chars[i] == '`' {
            flush_normal(&mut job, &mut current);
            i += 1;
            let mut code = String::new();
            while i < chars.len() && chars[i] != '`' {
                code.push(chars[i]);
                i += 1;
            }
            if i < chars.len() { i += 1; }
            if !code.is_empty() {
                job.append(" ", 0.0, egui::TextFormat {
                    font_id: mono(font_size - 1.0),
                    color: code_color,
                    background: code_bg,
                    ..Default::default()
                });
                job.append(&code, 0.0, egui::TextFormat {
                    font_id: mono(font_size - 1.0),
                    color: code_color,
                    background: code_bg,
                    ..Default::default()
                });
                job.append(" ", 0.0, egui::TextFormat {
                    font_id: mono(font_size - 1.0),
                    color: code_color,
                    background: code_bg,
                    ..Default::default()
                });
            }
        } else {
            current.push(chars[i]);
            i += 1;
        }
    }
    flush_normal(&mut job, &mut current);
    job
}
