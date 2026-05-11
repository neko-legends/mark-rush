use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use eframe::egui;
use eframe::egui::Widget;
use serde::{Deserialize, Serialize};

const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mdown", "mdx", "mkd", "rmd"];
const MAX_STORED_STATUS: usize = 180;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MarkRush")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([840.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "MarkRush",
        options,
        Box::new(|cc| Ok(Box::new(MarkRushApp::new(cc)))),
    )
}

#[derive(Clone, Serialize, Deserialize)]
struct AppSettings {
    last_root_path: Option<PathBuf>,
    last_file_path: Option<PathBuf>,
    theme_name: String,
    word_wrap: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            last_root_path: None,
            last_file_path: None,
            theme_name: "EVA Dark".to_owned(),
            word_wrap: true,
        }
    }
}

#[derive(Clone)]
struct AppTheme {
    name: &'static str,
    window: egui::Color32,
    panel: egui::Color32,
    panel_alt: egui::Color32,
    border: egui::Color32,
    text: egui::Color32,
    muted: egui::Color32,
    input: egui::Color32,
    accent: egui::Color32,
    accent_2: egui::Color32,
    button: egui::Color32,
    button_hover: egui::Color32,
    button_pressed: egui::Color32,
    search_highlight: egui::Color32,
    search_highlight_text: egui::Color32,
    code: egui::Color32,
}

#[derive(Clone)]
struct TreeEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
    loaded: bool,
    children: Vec<TreeEntry>,
}

impl TreeEntry {
    fn root(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        Self {
            path,
            name,
            is_dir: true,
            loaded: false,
            children: Vec::new(),
        }
    }

    fn load_children(&mut self) -> io::Result<()> {
        if !self.is_dir || self.loaded {
            return Ok(());
        }

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;
            let name = entry.file_name().to_string_lossy().to_string();

            if metadata.is_dir() {
                if is_visible_dir(&path) {
                    dirs.push(TreeEntry {
                        path,
                        name,
                        is_dir: true,
                        loaded: false,
                        children: Vec::new(),
                    });
                }
            } else if metadata.is_file() && is_markdown_file(&path) {
                files.push(TreeEntry {
                    path,
                    name,
                    is_dir: false,
                    loaded: true,
                    children: Vec::new(),
                });
            }
        }

        dirs.sort_by_key(|entry| entry.name.to_lowercase());
        files.sort_by_key(|entry| entry.name.to_lowercase());
        dirs.extend(files);
        self.children = dirs;
        self.loaded = true;
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderKind {
    Blank,
    Heading(usize),
    Paragraph,
    Quote,
    List,
    Code,
    Rule,
    Table,
}

#[derive(Clone)]
struct RenderLine {
    text: String,
    kind: RenderKind,
}

#[derive(Clone)]
struct SearchMatch {
    line_index: usize,
}

#[derive(Clone)]
enum PendingAction {
    OpenFile(PathBuf),
    SetRoot(PathBuf),
    ClearFile,
}

struct MarkRushApp {
    settings: AppSettings,
    settings_path: Option<PathBuf>,
    root: Option<TreeEntry>,
    current_file: Option<PathBuf>,
    document_text: String,
    saved_text: String,
    render_lines: Vec<RenderLine>,
    edit_mode: bool,
    dirty: bool,
    search_query: String,
    search_matches: Vec<SearchMatch>,
    current_match: usize,
    status: String,
    theme_index: usize,
    pending_action: Option<PendingAction>,
    show_save_prompt: bool,
    known_roots: HashSet<PathBuf>,
}

impl MarkRushApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_pixels_per_point(1.0);

        let settings_path = settings_path();
        let settings = load_settings(settings_path.as_deref());
        let theme_index = theme_index_by_name(&settings.theme_name);

        let mut app = Self {
            settings,
            settings_path,
            root: None,
            current_file: None,
            document_text: String::new(),
            saved_text: String::new(),
            render_lines: Vec::new(),
            edit_mode: false,
            dirty: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match: 0,
            status: "Ready".to_owned(),
            theme_index,
            pending_action: None,
            show_save_prompt: false,
            known_roots: HashSet::new(),
        };

        app.apply_startup_target();
        app
    }

    fn apply_startup_target(&mut self) {
        let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

        for arg in args {
            let path = absolute_path(&arg);
            if path.is_dir() {
                self.set_root(path);
                return;
            }

            if path.is_file() {
                let root = path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf();
                self.set_root(root);
                self.load_file(path);
                return;
            }
        }

        if let Some(root) = self
            .settings
            .last_root_path
            .clone()
            .filter(|path| path.is_dir())
        {
            self.set_root(root);

            if let Some(file) = self
                .settings
                .last_file_path
                .clone()
                .filter(|path| path.is_file())
            {
                self.load_file(file);
            }
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.apply_theme(ui.ctx());
        self.handle_shortcuts(ui);

        egui::Frame::new().fill(self.theme().window).show(ui, |ui| {
            ui.horizontal(|ui| {
                let height = ui.available_height();
                ui.allocate_ui_with_layout(
                    egui::vec2(300.0, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.draw_sidebar(ui),
                );

                ui.separator();

                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.draw_workspace(ui, frame),
                );
            });
        });

        if self.show_save_prompt {
            self.draw_save_prompt(ui.ctx());
        }
    }

    fn draw_sidebar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();

        egui::Frame::new()
            .fill(theme.panel)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Mark Rush")
                            .strong()
                            .color(theme.muted)
                            .size(13.0),
                    );

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if themed_button(ui, "Open Folder", &theme).clicked() {
                            self.open_folder_dialog();
                        }

                        if themed_button(ui, "Refresh", &theme).clicked() {
                            self.refresh_tree();
                        }
                    });

                    ui.add_space(8.0);
                    let root_label = self
                        .root
                        .as_ref()
                        .map(|entry| entry.path.to_string_lossy().to_string())
                        .unwrap_or_else(|| "No folder open".to_owned());
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(root_label)
                                .color(theme.muted)
                                .size(12.0),
                        )
                        .truncate(),
                    );

                    ui.add_space(8.0);

                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Some(root) = &mut self.root {
                                if let Some(selected) =
                                    show_tree(ui, root, self.current_file.as_deref(), &theme)
                                {
                                    self.request_open_file(selected);
                                }
                            } else {
                                ui.label(
                                    egui::RichText::new("Open a folder to browse Markdown files.")
                                        .color(theme.muted),
                                );
                            }
                        });
                });
            });
    }

    fn draw_workspace(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let theme = self.theme().clone();

        egui::Frame::new()
            .fill(theme.panel_alt)
            .inner_margin(egui::Margin::same(0))
            .show(ui, |ui| {
                self.draw_topbar(ui);
                ui.separator();
                self.draw_searchbar(ui);
                ui.separator();

                let available = ui.available_size();
                ui.allocate_ui(available, |ui| {
                    if self.edit_mode {
                        self.draw_editor(ui);
                    } else {
                        self.draw_reader(ui);
                    }
                });
            });
    }

    fn draw_topbar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();

        egui::Frame::new()
            .fill(theme.panel_alt)
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        let file_name = self
                            .current_file
                            .as_ref()
                            .and_then(|path| path.file_name())
                            .and_then(|name| name.to_str())
                            .unwrap_or("No file selected");
                        ui.label(
                            egui::RichText::new(file_name)
                                .strong()
                                .size(16.0)
                                .color(theme.text),
                        );

                        let file_path = self
                            .current_file
                            .as_ref()
                            .map(|path| path.to_string_lossy().to_string())
                            .unwrap_or_default();
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(file_path).size(12.0).color(theme.muted),
                            )
                            .truncate(),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut selected_theme = self.theme().name.to_owned();
                        egui::ComboBox::from_id_salt("theme-picker")
                            .width(145.0)
                            .selected_text(selected_theme.clone())
                            .show_ui(ui, |ui| {
                                for theme in themes() {
                                    ui.selectable_value(
                                        &mut selected_theme,
                                        theme.name.to_owned(),
                                        theme.name,
                                    );
                                }
                            });

                        if selected_theme != self.theme().name {
                            self.theme_index = theme_index_by_name(&selected_theme);
                            self.settings.theme_name = selected_theme;
                            self.save_settings();
                        }

                        let can_save =
                            self.current_file.is_some() && (self.dirty || self.edit_mode);
                        if ui
                            .add_enabled(can_save, egui::Button::new("Save").fill(theme.button))
                            .clicked()
                        {
                            self.save_current_file();
                        }

                        if ui
                            .add_enabled(
                                self.current_file.is_some(),
                                egui::Button::new("Reload").fill(theme.button),
                            )
                            .clicked()
                        {
                            if let Some(path) = self.current_file.clone() {
                                self.request_action(PendingAction::OpenFile(path));
                            }
                        }

                        let mut wrap = self.settings.word_wrap;
                        if ui.checkbox(&mut wrap, "Wrap").changed() {
                            self.settings.word_wrap = wrap;
                            self.save_settings();
                        }

                        let mut edit = self.edit_mode;
                        if ui.checkbox(&mut edit, "Edit").changed() {
                            self.edit_mode = edit;
                        }

                        if ui
                            .add_enabled(
                                self.current_file.is_some(),
                                egui::Button::new("Copy Path").fill(theme.button),
                            )
                            .clicked()
                        {
                            if let Some(path) = &self.current_file {
                                ui.ctx().copy_text(path.to_string_lossy().to_string());
                                self.set_status("Copied file path");
                            }
                        }
                    });
                });
            });
    }

    fn draw_searchbar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();

        egui::Frame::new()
            .fill(theme.panel_alt)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add_enabled(
                        self.current_file.is_some(),
                        egui::TextEdit::singleline(&mut self.search_query)
                            .desired_width(280.0)
                            .hint_text("Search"),
                    );

                    if response.changed() {
                        self.rebuild_search();
                    }

                    if ui
                        .add_enabled(
                            !self.search_matches.is_empty(),
                            egui::Button::new("Previous").fill(theme.button),
                        )
                        .clicked()
                    {
                        self.move_match(-1);
                    }

                    if ui
                        .add_enabled(
                            !self.search_matches.is_empty(),
                            egui::Button::new("Next").fill(theme.button),
                        )
                        .clicked()
                    {
                        self.move_match(1);
                    }

                    ui.label(egui::RichText::new(self.search_status()).color(theme.muted));
                });
            });
    }

    fn draw_reader(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();

        if self.current_file.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Open a Markdown file")
                        .color(theme.muted)
                        .size(18.0),
                );
            });
            return;
        }

        let current_match_line = self
            .search_matches
            .get(self.current_match)
            .map(|search_match| search_match.line_index);

        let row_height = 23.0;
        let scroll = egui::ScrollArea::vertical().auto_shrink([false, false]);

        scroll.show_rows(ui, row_height, self.render_lines.len(), |ui, row_range| {
            for index in row_range {
                if let Some(line) = self.render_lines.get(index) {
                    let is_match = current_match_line == Some(index);
                    draw_render_line(ui, line, &theme, is_match);
                }
            }
        });
    }

    fn draw_editor(&mut self, ui: &mut egui::Ui) {
        if self.current_file.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("Open a Markdown file to edit");
            });
            return;
        }

        let desired_width = if self.settings.word_wrap {
            ui.available_width()
        } else {
            12_000.0
        };

        let edit = egui::TextEdit::multiline(&mut self.document_text)
            .desired_width(desired_width)
            .code_editor()
            .lock_focus(true);

        let response = if self.settings.word_wrap {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| ui.add_sized(ui.available_size(), edit))
                .inner
        } else {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_sized(egui::vec2(desired_width, ui.available_height()), edit)
                })
                .inner
        };

        if response.changed() {
            self.dirty = self.document_text != self.saved_text;
            self.render_lines = parse_markdown_lines(&self.document_text);
            self.rebuild_search();
        }
    }

    fn draw_save_prompt(&mut self, ctx: &egui::Context) {
        let theme = self.theme().clone();
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("Save changes before continuing?");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if egui::Button::new("Save")
                        .fill(theme.button)
                        .ui(ui)
                        .clicked()
                    {
                        if self.save_current_file() {
                            self.finish_pending_action();
                        }
                    }

                    if egui::Button::new("Discard")
                        .fill(theme.button)
                        .ui(ui)
                        .clicked()
                    {
                        self.dirty = false;
                        self.finish_pending_action();
                    }

                    if egui::Button::new("Cancel")
                        .fill(theme.button)
                        .ui(ui)
                        .clicked()
                    {
                        self.pending_action = None;
                        self.show_save_prompt = false;
                    }
                });
            });
    }

    fn handle_shortcuts(&mut self, ui: &mut egui::Ui) {
        ui.input(|input| {
            if input.key_pressed(egui::Key::S) && input.modifiers.command {
                self.save_current_file();
            }

            if input.key_pressed(egui::Key::F) && input.modifiers.command {
                self.set_status("Search is ready");
            }

            if input.key_pressed(egui::Key::F5) {
                self.refresh_tree();
            }

            if input.key_pressed(egui::Key::F3) {
                if input.modifiers.shift {
                    self.move_match(-1);
                } else {
                    self.move_match(1);
                }
            }

            if input.key_pressed(egui::Key::Enter) && !self.search_query.is_empty() {
                if input.modifiers.shift {
                    self.move_match(-1);
                } else {
                    self.move_match(1);
                }
            }
        });
    }

    fn open_folder_dialog(&mut self) {
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            self.request_action(PendingAction::SetRoot(folder));
        }
    }

    fn request_open_file(&mut self, path: PathBuf) {
        if self.current_file.as_ref() == Some(&path) {
            return;
        }

        self.request_action(PendingAction::OpenFile(path));
    }

    fn request_action(&mut self, action: PendingAction) {
        if self.dirty {
            self.pending_action = Some(action);
            self.show_save_prompt = true;
        } else {
            self.apply_action(action);
        }
    }

    fn finish_pending_action(&mut self) {
        self.show_save_prompt = false;
        if let Some(action) = self.pending_action.take() {
            self.apply_action(action);
        }
    }

    fn apply_action(&mut self, action: PendingAction) {
        match action {
            PendingAction::OpenFile(path) => self.load_file(path),
            PendingAction::SetRoot(path) => {
                self.set_root(path);
                self.apply_action(PendingAction::ClearFile);
            }
            PendingAction::ClearFile => self.clear_file(),
        }
    }

    fn set_root(&mut self, path: PathBuf) {
        let root_path = absolute_path(&path);
        self.root = Some(TreeEntry::root(root_path.clone()));
        self.known_roots.insert(root_path.clone());
        self.settings.last_root_path = Some(root_path.clone());
        self.save_settings();
        self.set_status(format!("Opened folder {}", root_path.display()));
    }

    fn refresh_tree(&mut self) {
        if let Some(root) = self.root.as_ref().map(|entry| entry.path.clone()) {
            self.root = Some(TreeEntry::root(root));
            self.set_status("Folder tree refreshed");
        }
    }

    fn load_file(&mut self, path: PathBuf) {
        let full_path = absolute_path(&path);
        match fs::read(&full_path) {
            Ok(bytes) => {
                self.document_text = String::from_utf8_lossy(&bytes).to_string();
                self.saved_text = self.document_text.clone();
                self.render_lines = parse_markdown_lines(&self.document_text);
                self.current_file = Some(full_path.clone());
                self.dirty = false;
                self.edit_mode = false;
                self.settings.last_file_path = Some(full_path.clone());
                self.save_settings();
                self.rebuild_search();
                self.set_status(format!("Loaded {}", full_path.display()));
            }
            Err(error) => {
                self.set_status(format!("Could not load file: {error}"));
            }
        }
    }

    fn clear_file(&mut self) {
        self.current_file = None;
        self.document_text.clear();
        self.saved_text.clear();
        self.render_lines.clear();
        self.dirty = false;
        self.edit_mode = false;
        self.search_matches.clear();
        self.current_match = 0;
        self.settings.last_file_path = None;
        self.save_settings();
    }

    fn save_current_file(&mut self) -> bool {
        let Some(path) = self.current_file.clone() else {
            return false;
        };

        match fs::write(&path, self.document_text.as_bytes()) {
            Ok(()) => {
                self.saved_text = self.document_text.clone();
                self.dirty = false;
                self.render_lines = parse_markdown_lines(&self.document_text);
                self.rebuild_search();
                self.set_status("Saved");
                true
            }
            Err(error) => {
                self.set_status(format!("Save failed: {error}"));
                false
            }
        }
    }

    fn rebuild_search(&mut self) {
        self.search_matches.clear();
        self.current_match = 0;

        let query = self.search_query.trim().to_lowercase();
        if query.is_empty() {
            return;
        }

        for (line_index, line) in self.render_lines.iter().enumerate() {
            if line.text.to_lowercase().contains(&query) {
                self.search_matches.push(SearchMatch { line_index });
            }
        }
    }

    fn move_match(&mut self, direction: isize) {
        if self.search_matches.is_empty() {
            return;
        }

        let len = self.search_matches.len() as isize;
        let next = (self.current_match as isize + direction).rem_euclid(len);
        self.current_match = next as usize;
    }

    fn search_status(&self) -> String {
        if self.search_query.trim().is_empty() {
            "No search".to_owned()
        } else if self.search_matches.is_empty() {
            "No matches".to_owned()
        } else {
            format!(
                "{} of {}",
                self.current_match + 1,
                self.search_matches.len()
            )
        }
    }

    fn set_status(&mut self, status: impl Into<String>) {
        let mut status = status.into();
        if status.len() > MAX_STORED_STATUS {
            status.truncate(MAX_STORED_STATUS);
            status.push_str("...");
        }
        self.status = status;
    }

    fn theme(&self) -> &'static AppTheme {
        &themes()[self.theme_index]
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let theme = self.theme();
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = theme.panel_alt;
        visuals.panel_fill = theme.panel_alt;
        visuals.extreme_bg_color = theme.input;
        visuals.faint_bg_color = theme.panel;
        visuals.widgets.noninteractive.bg_fill = theme.panel_alt;
        visuals.widgets.noninteractive.bg_stroke.color = theme.border;
        visuals.widgets.noninteractive.fg_stroke.color = theme.text;
        visuals.widgets.inactive.bg_fill = theme.button;
        visuals.widgets.inactive.bg_stroke.color = theme.border;
        visuals.widgets.inactive.fg_stroke.color = theme.text;
        visuals.widgets.hovered.bg_fill = theme.button_hover;
        visuals.widgets.hovered.bg_stroke.color = theme.accent;
        visuals.widgets.hovered.fg_stroke.color = theme.text;
        visuals.widgets.active.bg_fill = theme.button_pressed;
        visuals.widgets.active.bg_stroke.color = theme.accent;
        visuals.widgets.active.fg_stroke.color = theme.text;
        visuals.hyperlink_color = theme.accent_2;
        visuals.selection.bg_fill = theme.accent;
        visuals.selection.stroke.color = theme.text;
        ctx.set_visuals(visuals);
    }

    fn save_settings(&self) {
        let Some(path) = &self.settings_path else {
            return;
        };

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if let Ok(json) = serde_json::to_string_pretty(&self.settings) {
            let _ = fs::write(path, json);
        }
    }
}

impl eframe::App for MarkRushApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.draw(ui, frame);
    }
}

fn show_tree(
    ui: &mut egui::Ui,
    entry: &mut TreeEntry,
    selected_file: Option<&Path>,
    theme: &AppTheme,
) -> Option<PathBuf> {
    if entry.is_dir {
        let response =
            egui::CollapsingHeader::new(egui::RichText::new(&entry.name).color(theme.text))
                .id_salt(entry.path.to_string_lossy().to_string())
                .show(ui, |ui| {
                    if let Err(error) = entry.load_children() {
                        ui.label(egui::RichText::new(error.to_string()).color(theme.muted));
                        return None;
                    }

                    for child in &mut entry.children {
                        if let Some(path) = show_tree(ui, child, selected_file, theme) {
                            return Some(path);
                        }
                    }

                    None
                });

        response.body_returned.flatten()
    } else {
        let selected = selected_file == Some(entry.path.as_path());
        let response = ui.selectable_label(
            selected,
            egui::RichText::new(&entry.name).color(if selected {
                theme.accent
            } else {
                theme.text
            }),
        );

        if response.clicked() {
            Some(entry.path.clone())
        } else {
            None
        }
    }
}

fn draw_render_line(ui: &mut egui::Ui, line: &RenderLine, theme: &AppTheme, is_match: bool) {
    let fill = if is_match {
        theme.search_highlight
    } else {
        egui::Color32::TRANSPARENT
    };

    egui::Frame::new().fill(fill).show(ui, |ui| {
        let text_color = if is_match {
            theme.search_highlight_text
        } else {
            theme.text
        };

        match line.kind {
            RenderKind::Blank => {
                ui.label("");
            }
            RenderKind::Heading(level) => {
                let size = match level {
                    1 => 22.0,
                    2 => 19.0,
                    3 => 17.0,
                    _ => 15.5,
                };
                ui.label(
                    egui::RichText::new(&line.text)
                        .strong()
                        .size(size)
                        .color(text_color),
                );
            }
            RenderKind::Quote => {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("|").color(theme.accent));
                    ui.label(egui::RichText::new(&line.text).italics().color(theme.muted));
                });
            }
            RenderKind::Code => {
                egui::Frame::new()
                    .fill(theme.code)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.monospace(egui::RichText::new(&line.text).color(text_color));
                    });
            }
            RenderKind::Rule => {
                ui.separator();
            }
            RenderKind::List | RenderKind::Table | RenderKind::Paragraph => {
                ui.label(egui::RichText::new(&line.text).color(text_color));
            }
        }
    });
}

fn themed_button(ui: &mut egui::Ui, label: &str, theme: &AppTheme) -> egui::Response {
    ui.add(egui::Button::new(label).fill(theme.button))
}

fn parse_markdown_lines(markdown: &str) -> Vec<RenderLine> {
    let mut lines = Vec::new();
    let mut in_code_block = false;

    for raw_line in markdown.lines() {
        let trimmed = raw_line.trim();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            lines.push(RenderLine {
                text: raw_line.to_owned(),
                kind: RenderKind::Code,
            });
            continue;
        }

        if in_code_block {
            lines.push(RenderLine {
                text: raw_line.to_owned(),
                kind: RenderKind::Code,
            });
            continue;
        }

        let (text, kind) = classify_markdown_line(raw_line);
        lines.push(RenderLine { text, kind });
    }

    if markdown.ends_with('\n') {
        lines.push(RenderLine {
            text: String::new(),
            kind: RenderKind::Blank,
        });
    }

    lines
}

fn classify_markdown_line(line: &str) -> (String, RenderKind) {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return (String::new(), RenderKind::Blank);
    }

    if let Some((level, text)) = parse_heading(trimmed) {
        return (text.to_owned(), RenderKind::Heading(level));
    }

    if is_horizontal_rule(trimmed) {
        return (String::new(), RenderKind::Rule);
    }

    if let Some(text) = trimmed.strip_prefix('>') {
        return (text.trim_start().to_owned(), RenderKind::Quote);
    }

    if is_list_item(trimmed) {
        return (line.to_owned(), RenderKind::List);
    }

    if trimmed.contains('|') {
        return (line.to_owned(), RenderKind::Table);
    }

    (line.to_owned(), RenderKind::Paragraph)
}

fn parse_heading(line: &str) -> Option<(usize, &str)> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if (1..=6).contains(&level) && line.chars().nth(level) == Some(' ') {
        Some((level, line[level + 1..].trim()))
    } else {
        None
    }
}

fn is_horizontal_rule(line: &str) -> bool {
    let normalized: String = line
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    normalized.len() >= 3
        && (normalized.chars().all(|character| character == '-')
            || normalized.chars().all(|character| character == '*')
            || normalized.chars().all(|character| character == '_'))
}

fn is_list_item(line: &str) -> bool {
    if line.starts_with("- ") || line.starts_with("* ") || line.starts_with("+ ") {
        return true;
    }

    let Some(dot_index) = line.find('.') else {
        return false;
    };

    dot_index > 0
        && dot_index < 8
        && line[..dot_index]
            .chars()
            .all(|character| character.is_ascii_digit())
        && line[dot_index + 1..].starts_with(' ')
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            MARKDOWN_EXTENSIONS
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(extension))
        })
        .unwrap_or(false)
}

fn is_visible_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| !name.starts_with('.'))
        .unwrap_or(true)
}

fn absolute_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn settings_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "flashosophy", "MarkRush")
        .map(|dirs| dirs.config_dir().join("settings.json"))
}

fn load_settings(path: Option<&Path>) -> AppSettings {
    let Some(path) = path else {
        return AppSettings::default();
    };

    fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn theme_index_by_name(name: &str) -> usize {
    themes()
        .iter()
        .position(|theme| theme.name.eq_ignore_ascii_case(name))
        .unwrap_or(0)
}

fn themes() -> &'static [AppTheme] {
    use egui::Color32;

    static THEMES: std::sync::OnceLock<Vec<AppTheme>> = std::sync::OnceLock::new();
    THEMES
        .get_or_init(|| {
            vec![
                AppTheme {
                    name: "EVA Dark",
                    window: hex("#1E1F22"),
                    panel: hex("#17181B"),
                    panel_alt: hex("#1E1F22"),
                    border: hex("#4E5058"),
                    text: hex("#DBDEE1"),
                    muted: hex("#949BA4"),
                    input: hex("#232428"),
                    accent: hex("#5865F2"),
                    accent_2: hex("#00A8FC"),
                    button: hex("#313338"),
                    button_hover: hex("#383A40"),
                    button_pressed: hex("#4149B8"),
                    search_highlight: hex("#F2A65A"),
                    search_highlight_text: Color32::BLACK,
                    code: hex("#232428"),
                },
                AppTheme {
                    name: "Pearl White",
                    window: hex("#F8F8FC"),
                    panel: hex("#ECECF4"),
                    panel_alt: hex("#F8F8FC"),
                    border: hex("#D0D0DC"),
                    text: hex("#1E1C2D"),
                    muted: hex("#72728A"),
                    input: Color32::WHITE,
                    accent: hex("#635BDC"),
                    accent_2: hex("#4478DC"),
                    button: hex("#F0F0F6"),
                    button_hover: hex("#E4E4EC"),
                    button_pressed: hex("#D8D7F4"),
                    search_highlight: hex("#F2A65A"),
                    search_highlight_text: hex("#1E1C2D"),
                    code: hex("#F0F0F6"),
                },
                AppTheme {
                    name: "Abyss Teal",
                    window: hex("#101415"),
                    panel: hex("#0B1011"),
                    panel_alt: hex("#101415"),
                    border: hex("#3A5257"),
                    text: hex("#D9EEEE"),
                    muted: hex("#93A5A8"),
                    input: hex("#161D1F"),
                    accent: hex("#2EC4B6"),
                    accent_2: hex("#F28F6F"),
                    button: hex("#1F2A2C"),
                    button_hover: hex("#28373A"),
                    button_pressed: hex("#286C68"),
                    search_highlight: hex("#F28F6F"),
                    search_highlight_text: hex("#101415"),
                    code: hex("#161D1F"),
                },
                AppTheme {
                    name: "Ember Glow",
                    window: hex("#18130F"),
                    panel: hex("#120F0C"),
                    panel_alt: hex("#18130F"),
                    border: hex("#604D3E"),
                    text: hex("#F1E7DC"),
                    muted: hex("#A4968B"),
                    input: hex("#241E18"),
                    accent: hex("#F2A65A"),
                    accent_2: hex("#6EBAF2"),
                    button: hex("#352C24"),
                    button_hover: hex("#45392E"),
                    button_pressed: hex("#755633"),
                    search_highlight: hex("#F2A65A"),
                    search_highlight_text: Color32::BLACK,
                    code: hex("#241E18"),
                },
                AppTheme {
                    name: "Mosswood",
                    window: hex("#101511"),
                    panel: hex("#0B100D"),
                    panel_alt: hex("#101511"),
                    border: hex("#3E5D4C"),
                    text: hex("#E2ECE6"),
                    muted: hex("#9AA89F"),
                    input: hex("#18201B"),
                    accent: hex("#4CC38A"),
                    accent_2: hex("#C69CF2"),
                    button: hex("#233028"),
                    button_hover: hex("#2C3D33"),
                    button_pressed: hex("#315B43"),
                    search_highlight: hex("#C69CF2"),
                    search_highlight_text: hex("#101511"),
                    code: hex("#18201B"),
                },
                AppTheme {
                    name: "Rose Noir",
                    window: hex("#140F10"),
                    panel: hex("#0E0A0B"),
                    panel_alt: hex("#140F10"),
                    border: hex("#5B4149"),
                    text: hex("#F0E3E8"),
                    muted: hex("#A19398"),
                    input: hex("#21191B"),
                    accent: hex("#E07383"),
                    accent_2: hex("#74D6C6"),
                    button: hex("#322629"),
                    button_hover: hex("#422F34"),
                    button_pressed: hex("#713946"),
                    search_highlight: hex("#F2A65A"),
                    search_highlight_text: Color32::BLACK,
                    code: hex("#21191B"),
                },
                AppTheme {
                    name: "High Contrast",
                    window: Color32::BLACK,
                    panel: hex("#080808"),
                    panel_alt: Color32::BLACK,
                    border: hex("#777777"),
                    text: Color32::WHITE,
                    muted: hex("#CFCFCF"),
                    input: Color32::BLACK,
                    accent: hex("#00E5FF"),
                    accent_2: hex("#FFFF00"),
                    button: hex("#101010"),
                    button_hover: hex("#1C1C1C"),
                    button_pressed: hex("#003F47"),
                    search_highlight: hex("#FFFF00"),
                    search_highlight_text: Color32::BLACK,
                    code: hex("#151515"),
                },
            ]
        })
        .as_slice()
}

fn hex(value: &str) -> egui::Color32 {
    let value = value.trim_start_matches('#');
    let r = u8::from_str_radix(&value[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&value[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&value[4..6], 16).unwrap_or(0);
    egui::Color32::from_rgb(r, g, b)
}
