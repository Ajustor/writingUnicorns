use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::editor::Editor;
use crate::filetree::FileTree;
use crate::git::GitStatus;
use crate::lsp::LspClient;
use crate::tabs::TabManager;
use crate::terminal::Terminal;
use crate::ui::layout::SidebarTab;
use crate::ui::palette::CommandPalette;
use crate::ui::shortcuts::ShortcutsHelp;
use crate::ui::statusbar::StatusBar;

pub struct WritingUnicorns {
    pub config: Config,
    pub tab_manager: TabManager,
    pub editor: Editor,
    pub file_tree: FileTree,
    pub git_status: GitStatus,
    pub lsp_client: Arc<Mutex<LspClient>>,
    pub terminal: Terminal,
    pub status_bar: StatusBar,
    pub command_palette: CommandPalette,
    pub shortcuts_help: ShortcutsHelp,
    pub sidebar_tab: SidebarTab,
    pub show_terminal: bool,
    pub show_sidebar: bool,
    pub sidebar_width: f32,
    pub terminal_height: f32,
    pub workspace_path: Option<PathBuf>,
    pub folder_pending: Option<std::sync::mpsc::Receiver<std::path::PathBuf>>,
    pub file_pending: Option<std::sync::mpsc::Receiver<std::path::PathBuf>>,
}

impl WritingUnicorns {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = Config::load();
        let lsp_client = Arc::new(Mutex::new(LspClient::new()));
        Self {
            config,
            tab_manager: TabManager::new(),
            editor: Editor::new(),
            file_tree: FileTree::new(),
            git_status: GitStatus::new(),
            lsp_client,
            terminal: Terminal::new(),
            status_bar: StatusBar::new(),
            command_palette: CommandPalette::new(),
            shortcuts_help: ShortcutsHelp::new(),
            sidebar_tab: SidebarTab::default(),
            show_terminal: true,
            show_sidebar: true,
            sidebar_width: 220.0,
            terminal_height: 200.0,
            workspace_path: None,
            folder_pending: None,
            file_pending: None,
        }
    }

    pub fn open_folder(&mut self, path: PathBuf) {
        self.workspace_path = Some(path.clone());
        self.file_tree.load(path.clone());
        self.git_status.load(path.clone());
    }

    pub fn open_file(&mut self, path: PathBuf) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            self.tab_manager.open(path.clone(), content.clone());
            self.editor.set_content(content, Some(path));
        }
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
}

impl eframe::App for WritingUnicorns {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // All global shortcuts in one ctx.input() call to avoid double-read
        let (
            want_open_folder,
            want_open_file,
            want_new,
            want_palette,
            want_terminal,
            want_sidebar,
            want_help,
        ) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::O) && i.modifiers.ctrl && !i.modifiers.shift,
                i.key_pressed(egui::Key::O) && i.modifiers.ctrl && i.modifiers.shift,
                i.key_pressed(egui::Key::N) && i.modifiers.ctrl,
                i.key_pressed(egui::Key::P) && i.modifiers.ctrl,
                i.key_pressed(egui::Key::Backtick) && i.modifiers.ctrl,
                i.key_pressed(egui::Key::B) && i.modifiers.ctrl,
                i.key_pressed(egui::Key::F1),
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

        crate::ui::layout::render(self, ctx);

        if self.command_palette.is_open() {
            self.command_palette
                .show(ctx, &mut self.file_tree, &mut self.workspace_path);
        }

        self.shortcuts_help.show(ctx);

        // Request continuous repaint while terminal is visible (for live output)
        if self.show_terminal {
            ctx.request_repaint();
        }
    }
}
