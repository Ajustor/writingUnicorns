use std::path::PathBuf;

use crate::config::Config;
use crate::editor::Editor;
use crate::filetree::FileTree;
use crate::git::GitStatus;
use crate::lsp::{LspClient, LspManager};
use crate::plugin::builtin::word_count::WordCountPlugin;
use crate::plugin::manager::PluginManager;
use crate::plugin::PluginContext;
use crate::runner::RunManager;
use crate::tabs::TabManager;
use crate::terminal::Terminal;
use crate::ui::layout::SidebarTab;
use crate::ui::palette::CommandPalette;
use crate::ui::run_panel::RunPanel;
use crate::ui::search::WorkspaceSearch;
use crate::ui::settings::SettingsPanel;
use crate::ui::shortcuts::ShortcutsHelp;
use crate::ui::statusbar::StatusBar;

pub struct WritingUnicorns {
    pub config: Config,
    pub tab_manager: TabManager,
    pub editor: Editor,
    pub file_tree: FileTree,
    pub git_status: GitStatus,
    pub lsp: LspManager,
    pub pending_hover_id: Option<u64>,
    pub lsp_hover_result: Option<String>,
    /// Pending LSP go-to-definition request id.
    pub pending_definition_id: Option<u64>,
    /// Pending LSP completion request id.
    pub pending_completion_id: Option<u64>,
    /// Last editor content_version sent to the LSP server (to detect changes).
    pub last_lsp_content_version: i32,
    pub terminals: Vec<Terminal>,
    pub active_terminal: usize,
    pub status_bar: StatusBar,
    pub command_palette: CommandPalette,
    pub shortcuts_help: ShortcutsHelp,
    pub settings_panel: SettingsPanel,
    pub sidebar_tab: SidebarTab,
    pub show_terminal: bool,
    pub show_sidebar: bool,
    pub sidebar_width: f32,
    pub terminal_height: f32,
    pub workspace_search: WorkspaceSearch,
    pub workspace_path: Option<PathBuf>,
    pub runner: RunManager,
    pub run_panel: RunPanel,
    pub folder_pending: Option<std::sync::mpsc::Receiver<std::path::PathBuf>>,
    pub file_pending: Option<std::sync::mpsc::Receiver<std::path::PathBuf>>,
    pub plugin_manager: PluginManager,
    pub plugin_status: Option<String>,
    pub extension_registry: crate::extension::registry::ExtensionRegistry,
    pub extensions_panel: crate::extension::ui::ExtensionsPanel,
    /// Whether the "unsaved files" quit dialog is showing.
    pub show_close_warning: bool,
    /// Set to true after user confirms quitting — lets the next close go through.
    confirmed_close: bool,
    /// Tab id pending Ctrl+W close with unsaved warning.
    pub close_tab_id_pending: Option<usize>,
}

impl WritingUnicorns {
    pub fn new(_cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
        let config = Config::load();
        let mut plugin_manager = PluginManager::new();
        plugin_manager.register(Box::new(WordCountPlugin::new()));
        plugin_manager.register(Box::new(
            crate::extension::builtin::rust_lang::RustLangExtension,
        ));
        plugin_manager.register(Box::new(
            crate::extension::builtin::web_lang::WebLangExtension,
        ));
        plugin_manager.register(Box::new(
            crate::extension::builtin::python_lang::PythonLangExtension,
        ));
        plugin_manager.register(Box::new(
            crate::extension::builtin::data_lang::DataLangExtension,
        ));
        plugin_manager.register(Box::new(
            crate::extension::builtin::shell_lang::ShellLangExtension,
        ));
        plugin_manager.register(Box::new(
            crate::extension::builtin::docker_lang::DockerLangExtension,
        ));
        let mut extension_registry = crate::extension::registry::ExtensionRegistry::new();
        extension_registry.load_installed();
        let initial_terminal_height = config.terminal_height;
        let mut app = Self {
            config,
            tab_manager: TabManager::new(),
            editor: Editor::new(),
            file_tree: FileTree::new(),
            git_status: GitStatus::new(),
            lsp: LspManager::new(),
            pending_hover_id: None,
            lsp_hover_result: None,
            pending_definition_id: None,
            pending_completion_id: None,
            last_lsp_content_version: 0,
            terminals: vec![Terminal::new()],
            active_terminal: 0,
            status_bar: StatusBar::new(),
            command_palette: CommandPalette::new(),
            shortcuts_help: ShortcutsHelp::new(),
            settings_panel: SettingsPanel::new(),
            sidebar_tab: SidebarTab::default(),
            show_terminal: true,
            show_sidebar: true,
            sidebar_width: 220.0,
            terminal_height: initial_terminal_height,
            workspace_search: WorkspaceSearch::new(),
            workspace_path: None,
            runner: RunManager::new(),
            run_panel: RunPanel::new(),
            folder_pending: None,
            file_pending: None,
            plugin_manager,
            plugin_status: None,
            extension_registry,
            extensions_panel: crate::extension::ui::ExtensionsPanel::new(),
            show_close_warning: false,
            confirmed_close: false,
            close_tab_id_pending: None,
        };

        if let Some(path) = initial_path {
            // CLI argument takes priority — open directly, skip saved state.
            if path.is_dir() {
                app.open_folder(path);
            } else if path.is_file() {
                app.open_file(path);
            }
        } else {
            // No CLI arg: restore last workspace and last file from config.
            if let Some(ws_str) = app.config.last_workspace.clone() {
                let ws_path = PathBuf::from(&ws_str);
                if ws_path.is_dir() {
                    app.workspace_path = Some(ws_path.clone());
                    app.file_tree.load(ws_path.clone());
                    app.git_status.load(ws_path.clone());
                    app.runner.load_for_workspace(&ws_path);
                }
            }
            if let Some(file_str) = app.config.last_file.clone() {
                let file_path = PathBuf::from(&file_str);
                if file_path.is_file() {
                    // Read directly to avoid a redundant config save on startup.
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        app.tab_manager.open(file_path.clone(), content.clone());
                        app.editor.set_content(content, Some(file_path));
                    }
                }
            }
        }

        app
    }

    pub fn run_active_config(&mut self) {
        let workspace = self.workspace_path.clone();
        let current_file = self.editor.current_path.clone();

        if let Some(cmd) = self
            .runner
            .build_command(workspace.as_deref(), current_file.as_deref())
        {
            self.show_terminal = true;
            if self.terminal_height < 150.0 {
                self.terminal_height = 250.0;
            }
            if let Some(terminal) = self.terminals.get_mut(self.active_terminal) {
                terminal.send_input(&cmd);
                terminal.scroll_to_bottom();
            }
            self.runner.is_running = true;
        }
    }

    pub fn open_file_at_line(&mut self, path: PathBuf, line: usize) {
        self.open_file(path);
        let max = self.editor.buffer.num_lines().saturating_sub(1);
        self.editor.cursor.set_position(line.min(max), 0);
        self.editor.scroll_to_cursor = true;
    }

    pub fn open_folder(&mut self, path: PathBuf) {
        self.workspace_path = Some(path.clone());
        self.file_tree.load(path.clone());
        self.git_status.load(path.clone());
        self.runner.load_for_workspace(&path);
        self.config.last_workspace = Some(path.to_string_lossy().to_string());
        self.config.save();
    }

    pub fn open_file(&mut self, path: PathBuf) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            self.tab_manager.open(path.clone(), content.clone());
            self.editor.set_content(content.clone(), Some(path.clone()));
            self.config.last_file = Some(path.to_string_lossy().to_string());
            self.config.save();
            self.ensure_lsp_for_file(&path);
            // Notify LSP server that a file was opened.
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let lang_id = match ext {
                    "rs" => "rust",
                    "ts" | "tsx" => "typescript",
                    "js" | "jsx" => "javascript",
                    "py" => "python",
                    _ => ext,
                };
                let uri = format!("file://{}", path.display());
                if let Some(client) = self.lsp.get_mut(ext) {
                    client.did_open(&uri, lang_id, &content);
                }
            }
            self.last_lsp_content_version = 0;
        }
    }

    /// Notify the LSP server that the file content changed.
    pub fn notify_lsp_change(&mut self, path: &std::path::Path, content: &str, version: i32) {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(client) = self.lsp.get_mut(ext) {
                let uri = format!("file://{}", path.display());
                client.did_change(&uri, version, content);
            }
        }
    }

    fn ensure_lsp_for_file(&mut self, path: &std::path::Path) {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(workspace) = self.workspace_path.clone() {
                self.lsp.ensure_started(ext, &workspace);
            }
        }
    }

    /// Request hover information from the LSP server.
    /// Returns the request id, or `None` if no LSP is connected for this file.
    pub fn lsp_hover(&mut self, path: &std::path::Path, line: u32, col: u32) -> Option<u64> {
        let ext = path.extension()?.to_str()?;
        let client = self.lsp.get_mut(ext)?;
        if !client.is_connected {
            return None;
        }
        let uri = format!("file://{}", path.display());
        Some(client.request_hover(&uri, line, col))
    }
    pub fn open_new_file(&mut self) {
        self.editor.set_content(String::new(), None);
        self.tab_manager.open_untitled();
    }

    pub fn trigger_open_folder(&mut self) -> std::sync::mpsc::Receiver<PathBuf> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                let _ = tx.send(path);
            }
        });
        rx
    }

    pub fn trigger_open_file(&mut self) -> std::sync::mpsc::Receiver<PathBuf> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                let _ = tx.send(path);
            }
        });
        rx
    }

    /// Load the currently active tab into the editor, or clear the editor if none.
    pub fn load_active_tab(&mut self) {
        if let Some(id) = self.tab_manager.active_tab {
            if let Some(tab) = self.tab_manager.tabs.iter().find(|t| t.id == id) {
                let path = tab.path.clone();
                if let Ok(content) = std::fs::read_to_string(&path) {
                    self.editor.set_content(content, Some(path));
                }
            }
        } else {
            self.editor.set_content(String::new(), None);
        }
    }

    /// Navigate to the definition of `word` via LSP (async), then file-path lookup, then workspace search.
    pub fn handle_go_to_definition(&mut self, word: &str) {
        // Strategy 0: try LSP definition (async — will navigate when the response arrives).
        if let Some(path) = self.editor.current_path.clone() {
            let (row, col) = self.editor.cursor.position();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if let Some(client) = self.lsp.get_mut(ext) {
                    if client.is_connected {
                        let uri = format!("file://{}", path.display());
                        self.pending_definition_id =
                            Some(client.request_definition(&uri, row as u32, col as u32));
                        // LSP result will navigate via poll; also run regex so we don't stall.
                    }
                }
            }
        }
        self.handle_go_to_definition_regex(word);
    }

    /// Regex/workspace-search-based go-to-definition (synchronous fallback).
    pub fn handle_go_to_definition_regex(&mut self, word: &str) {
        // Strategy 0: search current file first (fastest, most likely match).
        let current_content = self.editor.buffer.to_string();
        if let Some((_, line)) = find_definition_in_buffer(&current_content, word) {
            let max = self.editor.buffer.num_lines().saturating_sub(1);
            self.editor.cursor.set_position(line.min(max), 0);
            self.editor.scroll_to_cursor = true;
            return;
        }

        // Strategy 1: looks like a file path — try resolving it.
        let base_dirs: Vec<PathBuf> = [
            self.editor
                .current_path
                .as_ref()
                .and_then(|p| p.parent().map(|p| p.to_path_buf())),
            self.workspace_path.clone(),
        ]
        .into_iter()
        .flatten()
        .collect();

        let extensions = [
            "", ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".toml",
        ];

        for base in &base_dirs {
            for ext in &extensions {
                let candidate = base.join(format!("{}{}", word, ext));
                if candidate.is_file() {
                    self.open_file(candidate);
                    return;
                }
            }
        }

        // Strategy 2: search workspace for a definition.
        // Build patterns (most specific first to avoid false positives from the bare fallback).
        let patterns: Vec<String> = vec![
            format!("fn {}(", word),
            format!("fn {} (", word),
            format!("pub fn {}(", word),
            format!("pub fn {} (", word),
            format!("pub async fn {}(", word),
            format!("async fn {}(", word),
            format!("fn {}(&self", word),
            format!("fn {}(&mut self", word),
            format!("struct {}", word),
            format!("enum {}", word),
            format!("trait {}", word),
            format!("class {}", word),
            format!("interface {}", word),
            format!("type {} =", word),
            format!("const {}", word),
            format!("let {} =", word),
            format!("def {}(", word),
            format!("function {}(", word),
            format!("export function {}(", word),
            format!("export class {}", word),
            format!("fn {}", word), // fallback
        ];
        if let Some(ws) = self.workspace_path.clone() {
            // Prefer same directory as the current file for faster, more relevant results.
            let current_dir = self
                .editor
                .current_path
                .as_ref()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()));
            if let Some(dir) = current_dir {
                if let Some((path, line)) =
                    search_workspace_for_symbol(&dir, &patterns, 200, 3)
                {
                    self.open_file_at_line(path, line);
                    return;
                }
            }
            // Fall back to full workspace search with higher limits.
            if let Some((path, line)) = search_workspace_for_symbol(&ws, &patterns, 2000, 10) {
                self.open_file_at_line(path, line);
            }
        }
    }
}

/// Walk `workspace` searching for any of `patterns` in source files.
///
/// Returns the path and 0-indexed line number of the first match found.
/// Files in each directory are checked before recursing into subdirectories so that
/// shallower (more relevant) definitions are found first.
fn search_workspace_for_symbol(
    workspace: &std::path::Path,
    patterns: &[String],
    max_files: usize,
    max_depth: usize,
) -> Option<(PathBuf, usize)> {
    let mut file_count = 0usize;
    search_in_dir(
        workspace,
        patterns,
        0,
        max_depth,
        &mut file_count,
        max_files,
    )
}

fn search_in_dir(
    dir: &std::path::Path,
    patterns: &[String],
    depth: usize,
    max_depth: usize,
    file_count: &mut usize,
    max_files: usize,
) -> Option<(PathBuf, usize)> {
    if depth > max_depth {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    let mut subdirs: Vec<PathBuf> = Vec::new();
    let mut source_files: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.')
            || matches!(name_str.as_ref(), "target" | "node_modules" | ".git")
        {
            continue;
        }
        if path.is_dir() {
            subdirs.push(path);
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(
                ext,
                "rs" | "ts"
                    | "tsx"
                    | "js"
                    | "jsx"
                    | "py"
                    | "go"
                    | "java"
                    | "kt"
                    | "c"
                    | "cpp"
                    | "h"
            ) {
                source_files.push(path);
            }
        }
    }
    // Search files in the current directory first, then recurse into subdirectories.
    for path in source_files {
        *file_count += 1;
        if *file_count > max_files {
            return None;
        }
        if let Some(line) = search_file_for_patterns(&path, patterns) {
            return Some((path, line));
        }
    }
    for subdir in subdirs {
        if let Some(result) = search_in_dir(
            &subdir,
            patterns,
            depth + 1,
            max_depth,
            file_count,
            max_files,
        ) {
            return Some(result);
        }
    }
    None
}

/// Return the 0-indexed line number of the first line in `path` that contains any of `patterns`.
fn search_file_for_patterns(path: &std::path::Path, patterns: &[String]) -> Option<usize> {
    let content = std::fs::read_to_string(path).ok()?;
    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        for pattern in patterns {
            if trimmed.contains(pattern.as_str()) {
                return Some(line_idx);
            }
        }
    }
    None
}

/// Returns true if `line` contains `word` as a definition token with proper word boundaries.
fn contains_as_definition(line: &str, pattern: &str, word: &str) -> bool {
    let Some(pat_pos) = line.find(pattern) else {
        return false;
    };
    // Verify the word within the pattern has word boundaries.
    let Some(word_pos) = line[pat_pos..].find(word).map(|p| pat_pos + p) else {
        return false;
    };
    let bytes = line.as_bytes();
    let before = bytes
        .get(word_pos.saturating_sub(1))
        .copied()
        .unwrap_or(b' ');
    let after = bytes.get(word_pos + word.len()).copied().unwrap_or(b' ');
    let before_ok = !before.is_ascii_alphanumeric() && before != b'_';
    let after_ok = !after.is_ascii_alphanumeric() && after != b'_';
    before_ok && after_ok
}

fn find_definition_in_buffer(content: &str, word: &str) -> Option<(String, usize)> {
    let patterns: &[String] = &[
        // Rust — bare fn
        format!("fn {}(", word),
        format!("fn {} (", word),
        // Rust — visibility + fn
        format!("pub fn {}(", word),
        format!("pub fn {} (", word),
        format!("pub async fn {}(", word),
        format!("pub async fn {} (", word),
        format!("pub(crate) fn {}(", word),
        format!("pub(super) fn {}(", word),
        format!("pub unsafe fn {}(", word),
        // Rust — other fn flavours
        format!("async fn {}(", word),
        format!("async fn {} (", word),
        format!("const fn {}(", word),
        format!("unsafe fn {}(", word),
        // Rust — impl-block methods (indented)
        format!("  fn {}(", word),
        format!("    fn {}(", word),
        format!("fn {}(&", word),
        format!("fn {}(&mut", word),
        // Rust — type-level definitions
        format!("struct {}", word),
        format!("enum {}", word),
        format!("trait {}", word),
        format!("impl {}", word),
        format!("type {} =", word),
        format!("const {}", word),
        format!("let {} =", word),
        format!("macro_rules! {}", word),
        // JavaScript / TypeScript
        format!("function {}(", word),
        format!("function {} (", word),
        format!("class {}", word),
        format!("interface {}", word),
        format!("export function {}", word),
        format!("export class {}", word),
        format!("export const {} =", word),
        format!("export default function {}", word),
        format!("get {}(", word),
        format!("set {}(", word),
        format!("async {}(", word),
        format!("{}:", word),
        // Python
        format!("def {}(", word),
        format!("def {} (", word),
        format!("async def {}(", word),
        format!("  def {}(", word),
        format!("    def {}(", word),
    ];

    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        for pattern in patterns {
            if contains_as_definition(trimmed, pattern, word) {
                return Some((line.to_string(), line_idx));
            }
        }
    }
    None
}

impl eframe::App for WritingUnicorns {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle window close request — warn about unsaved files
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        let any_unsaved = self.tab_manager.tabs.iter().any(|t| t.is_modified);
        if close_requested && any_unsaved && !self.confirmed_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.show_close_warning = true;
        }

        // App-quit unsaved-changes dialog
        if self.show_close_warning {
            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("You have unsaved files. What would you like to do?");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save All & Quit").clicked() {
                            let _ = self.editor.save();
                            self.confirmed_close = true;
                            self.show_close_warning = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("Quit Without Saving").clicked() {
                            self.confirmed_close = true;
                            self.show_close_warning = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_close_warning = false;
                        }
                    });
                });
        }

        // Ctrl+W close-tab unsaved-changes dialog
        if let Some(pending_id) = self.close_tab_id_pending {
            egui::Window::new("Close Tab")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    let tab_title = self
                        .tab_manager
                        .tabs
                        .iter()
                        .find(|t| t.id == pending_id)
                        .map(|t| t.title.clone())
                        .unwrap_or_default();
                    ui.label(format!("\"{}\" has unsaved changes.", tab_title));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save & Close").clicked() {
                            let _ = self.editor.save();
                            self.tab_manager.close(pending_id);
                            self.close_tab_id_pending = None;
                            self.load_active_tab();
                        }
                        if ui.button("Discard & Close").clicked() {
                            self.tab_manager.close(pending_id);
                            self.close_tab_id_pending = None;
                            self.load_active_tab();
                        }
                        if ui.button("Cancel").clicked() {
                            self.close_tab_id_pending = None;
                        }
                    });
                });
        }

        // All global shortcuts in one ctx.input() call to avoid double-read
        let (
            want_open_folder,
            want_open_file,
            want_new,
            want_palette,
            want_terminal,
            want_sidebar,
            want_help,
            want_settings,
            want_search,
            want_close_tab,
            want_run,
        ) = ctx.input(|i| {
            (
                self.config.keybindings.open_folder.matches(i),
                self.config.keybindings.open_file.matches(i),
                self.config.keybindings.new_file.matches(i),
                self.config.keybindings.command_palette.matches(i),
                self.config.keybindings.toggle_terminal.matches(i),
                self.config.keybindings.toggle_sidebar.matches(i),
                self.config.keybindings.shortcuts_help.matches(i),
                self.config.keybindings.settings.matches(i),
                i.key_pressed(egui::Key::F) && i.modifiers.ctrl && i.modifiers.shift,
                self.config.keybindings.close_tab.matches(i),
                i.key_pressed(egui::Key::F5),
            )
        });

        if want_open_folder {
            self.folder_pending = Some(self.trigger_open_folder());
        }
        if want_open_file {
            self.file_pending = Some(self.trigger_open_file());
        }
        if want_new {
            self.open_new_file();
        }
        if want_palette {
            self.command_palette.toggle();
        }
        if want_terminal {
            self.show_terminal = !self.show_terminal;
        }
        if want_sidebar {
            self.show_sidebar = !self.show_sidebar;
        }
        if want_help {
            self.shortcuts_help.toggle();
        }
        if want_settings {
            self.settings_panel.toggle();
        }
        if want_search {
            self.show_sidebar = true;
            self.sidebar_tab = SidebarTab::Search;
        }
        if want_run {
            self.run_active_config();
        }
        if want_close_tab && self.close_tab_id_pending.is_none() {
            if let Some(id) = self.tab_manager.active_tab {
                let is_modified = self
                    .tab_manager
                    .tabs
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| t.is_modified)
                    .unwrap_or(false);
                if is_modified {
                    self.close_tab_id_pending = Some(id);
                } else {
                    self.tab_manager.close(id);
                    self.load_active_tab();
                }
            }
        }

        // Poll all LSP clients for incoming messages.
        let lsp_responses = self.lsp.poll_all();
        for (_ext, msgs) in lsp_responses {
            for (id, response) in msgs {
                if Some(id) == self.pending_hover_id {
                    self.lsp_hover_result = LspClient::parse_hover(&response);
                    self.pending_hover_id = None;
                } else if Some(id) == self.pending_definition_id {
                    if let Some((path, line)) = LspClient::parse_definition(&response) {
                        self.open_file_at_line(path, line as usize);
                        self.pending_definition_id = None;
                    }
                } else if Some(id) == self.pending_completion_id {
                    let items = LspClient::parse_completions(&response);
                    if !items.is_empty() {
                        self.editor.autocomplete.set_lsp_suggestions(
                            items.iter().map(|i| i.label.clone()).collect(),
                        );
                    }
                    self.pending_completion_id = None;
                }
            }
        }

        // Trigger an LSP hover request if the editor signals one is needed.
        if self.editor.hover_lsp_request_pending && self.pending_hover_id.is_none() {
            if let Some(path) = self.editor.current_path.clone() {
                let row = self.editor.hover_row;
                let col = self.editor.hover_col;
                self.pending_hover_id = self.lsp_hover(&path, row, col);
            }
        }

        // Send didChange to LSP when the buffer was modified.
        if self.editor.content_version != self.last_lsp_content_version {
            if let Some(path) = self.editor.current_path.clone() {
                let content = self.editor.buffer.to_string();
                let version = self.editor.content_version;
                self.notify_lsp_change(&path, &content, version);
                self.last_lsp_content_version = version;
            }
        }

        // Update diagnostics for the current file.
        if let Some(path) = self.editor.current_path.clone() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let uri = format!("file://{}", path.display());
                if let Some(client) = self.lsp.get(ext) {
                    self.editor.diagnostics = client.get_diagnostics(&uri);
                }
            }
        }

        // Handle Ctrl+Space LSP completion request from the editor.
        if self.editor.completion_request_pending {
            self.editor.completion_request_pending = false;
            if let Some(path) = self.editor.current_path.clone() {
                let row = self.editor.completion_trigger_row;
                let col = self.editor.completion_trigger_col;
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if let Some(client) = self.lsp.get_mut(ext) {
                        if client.is_connected {
                            let uri = format!("file://{}", path.display());
                            self.pending_completion_id =
                                Some(client.request_completions(&uri, row as u32, col as u32));
                        }
                    }
                }
            }
        }

        crate::ui::layout::render(self, ctx);

        // Handle Ctrl+click go-to-definition request emitted by the editor.
        if let Some(word) = self.editor.go_to_definition_request.take() {
            self.handle_go_to_definition(&word);
        }

        // Run plugins each frame and collect their status text.
        let buffer_text = self.editor.buffer.to_string();
        let filename = self.editor.current_path.as_ref().and_then(|p| p.to_str());
        let plugin_ctx = PluginContext {
            buffer_text: &buffer_text,
            filename,
            cursor_row: self.editor.cursor.row,
            cursor_col: self.editor.cursor.col,
            is_modified: self.editor.is_modified,
            hovered_word: self.editor.hovered_word(),
        };
        let responses = self.plugin_manager.update_all(&plugin_ctx);
        self.plugin_status = responses
            .into_iter()
            .filter_map(|r| r.status_text)
            .next_back();

        if self.command_palette.is_open() {
            self.command_palette
                .show(ctx, &mut self.file_tree, &mut self.workspace_path);
        }

        self.shortcuts_help.show(ctx, &self.config.keybindings);

        if self.settings_panel.show(ctx, &mut self.config) {
            self.config.save();
        }

        // Auto-save on focus loss when enabled
        if self.config.editor.auto_save && self.editor.is_modified {
            let window_focused = ctx.input(|i| i.focused);
            if !window_focused {
                let _ = self.editor.save();
            }
        }

        // Request continuous repaint while terminal is visible (for live output)
        if self.show_terminal {
            ctx.request_repaint();
        }
    }
}
