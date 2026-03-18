use egui_phosphor::regular as ph;

/// Returns (phosphor icon char, color) for a given filename.
pub fn file_icon(name: &str) -> (&'static str, egui::Color32) {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => (ph::FILE_RS, egui::Color32::from_rgb(222, 99, 52)),
        "py" => (ph::FILE_PY, egui::Color32::from_rgb(53, 114, 165)),
        "js" | "mjs" | "cjs" => (ph::FILE_JS, egui::Color32::from_rgb(240, 219, 79)),
        "ts" => (ph::FILE_TS, egui::Color32::from_rgb(49, 120, 198)),
        "jsx" => (ph::FILE_JSX, egui::Color32::from_rgb(97, 218, 251)),
        "tsx" => (ph::FILE_TSX, egui::Color32::from_rgb(97, 218, 251)),
        "json" | "jsonc" => (ph::BRACKETS_CURLY, egui::Color32::from_rgb(255, 196, 88)),
        "toml" => (ph::FILE_CODE, egui::Color32::from_rgb(156, 220, 254)),
        "yaml" | "yml" => (ph::FILE_CODE, egui::Color32::from_rgb(206, 145, 120)),
        "md" | "mdx" => (ph::FILE_MD, egui::Color32::from_rgb(100, 200, 255)),
        "html" | "htm" => (ph::FILE_HTML, egui::Color32::from_rgb(228, 79, 38)),
        "css" => (ph::FILE_CSS, egui::Color32::from_rgb(86, 156, 214)),
        "scss" | "sass" | "less" => (ph::FILE_CSS, egui::Color32::from_rgb(205, 103, 153)),
        "c" | "h" => (ph::FILE_C, egui::Color32::from_rgb(85, 144, 196)),
        "cpp" | "cc" | "cxx" | "hpp" => (ph::FILE_CPP, egui::Color32::from_rgb(85, 144, 196)),
        "sql" => (ph::FILE_SQL, egui::Color32::from_rgb(218, 160, 17)),
        "svg" => (ph::FILE_SVG, egui::Color32::from_rgb(255, 160, 40)),
        "xml" => (ph::FILE_CODE, egui::Color32::from_rgb(228, 79, 38)),
        "sh" | "bash" | "zsh" | "fish" => (ph::TERMINAL, egui::Color32::from_rgb(35, 209, 139)),
        "txt" | "log" => (ph::FILE_TXT, egui::Color32::GRAY),
        "lock" => (ph::FILE_LOCK, egui::Color32::GRAY),
        "go" => (ph::FILE_CODE, egui::Color32::from_rgb(0, 173, 216)),
        "java" => (ph::FILE_CODE, egui::Color32::from_rgb(176, 114, 25)),
        "kt" | "kts" => (ph::FILE_CODE, egui::Color32::from_rgb(169, 121, 227)),
        "swift" => (ph::FILE_CODE, egui::Color32::from_rgb(240, 81, 56)),
        "rb" => (ph::FILE_CODE, egui::Color32::from_rgb(204, 52, 45)),
        "php" => (ph::FILE_CODE, egui::Color32::from_rgb(119, 123, 179)),
        "lua" => (ph::FILE_CODE, egui::Color32::from_rgb(80, 80, 228)),
        "cs" => (ph::FILE_C_SHARP, egui::Color32::from_rgb(104, 33, 122)),
        "dart" => (ph::FILE_CODE, egui::Color32::from_rgb(84, 182, 217)),
        "zig" => (ph::FILE_CODE, egui::Color32::from_rgb(247, 175, 48)),
        "ex" | "exs" => (ph::FILE_CODE, egui::Color32::from_rgb(102, 51, 153)),
        _ => (ph::FILE, egui::Color32::from_gray(160)),
    }
}
