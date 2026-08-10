#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use directories::ProjectDirs;
use eframe::egui;
use eframe::egui::Widget;
use serde::{Deserialize, Serialize};

const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mdown", "mdx", "mkd", "rmd"];
const APP_BUILD_LABEL: &str = env!("MARKRUSH_BUILD_LABEL");
const MAX_STORED_STATUS: usize = 180;
const MAX_STORED_FILE_VIEW_STATES: usize = 250;
const MIDDLE_SCROLL_DEAD_ZONE: f32 = 8.0;
const MIDDLE_SCROLL_MAX_SPEED: f32 = 80.0;
const SELECTION_SCROLL_DEAD_ZONE: f32 = 20.0;
const SELECTION_SCROLL_MAX_SPEED: f32 = 95.0;
const SELECTION_SCROLL_ZONE_MULTIPLIER: f32 = 3.5;
const ANIMATION_FRAME_INTERVAL: Duration = Duration::from_millis(33);
const DEFAULT_DOCUMENT_FONT_FAMILY: &str = "System UI";
const DEFAULT_DOCUMENT_FONT_SIZE: f32 = 15.0;
const FONT_SIZE_OPTIONS: &[f32] = &[12.0, 13.0, 14.0, 15.0, 16.0, 18.0, 20.0, 22.0, 24.0];
const MIN_WINDOW_SIZE: [f32; 2] = [840.0, 520.0];
const FIRST_LAUNCH_WINDOW_SCREEN_SCALE: f32 = 0.5;
const PORTRAIT_WINDOW_WIDTH_RATIO: f32 = 0.9;
const UNWRAPPED_DOCUMENT_WIDTH: f32 = 12_000.0;
const TOPBAR_PATH_COPY_WIDTH: f32 = 30.0;
const TOPBAR_LEFT_CONTROLS_WIDTH: f32 = 220.0;
const TOPBAR_TOOLBAR_HEIGHT: f32 = 34.0;
const TOPBAR_ICON_BUTTON_SIZE: f32 = 32.0;
const TOPBAR_MIN_FONT_SIZE_WIDTH: f32 = 64.0;
const TOPBAR_MIN_FONT_WIDTH: f32 = 96.0;
const TOPBAR_MIN_THEME_WIDTH: f32 = 92.0;
const TOPBAR_FONT_SIZE_WIDTH: f32 = 82.0;
const TOPBAR_FONT_WIDTH: f32 = 130.0;
const TOPBAR_THEME_WIDTH: f32 = 145.0;
const TOPBAR_FILE_ACTION_COUNT: f32 = 3.0;
const CODE_BLOCK_COPY_GUTTER_WIDTH: f32 = 32.0;
const CODE_BLOCK_COPY_BUTTON_SIZE: f32 = 24.0;
const CODE_COPY_FEEDBACK_DURATION: Duration = Duration::from_millis(1_200);
const READER_HORIZONTAL_PADDING: f32 = 36.0;
const READER_MIN_OVERSCAN: f32 = 900.0;
const READER_OVERSCAN_VIEWPORTS: f32 = 1.5;
const READER_MAX_OVERSCAN: f32 = 6_000.0;
const PRINT_PREVIEW_CHARS_PER_LINE: usize = 88;
const PRINT_PREVIEW_LINES_PER_PAGE: usize = 54;

fn main() -> eframe::Result {
    install_panic_hook();
    let settings = load_settings(settings_path().as_deref());
    let window_size = startup_window_size(&settings);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MarkRush")
            .with_inner_size(window_size)
            .with_min_inner_size(MIN_WINDOW_SIZE)
            .with_icon(load_app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "MarkRush",
        options,
        Box::new(|cc| Ok(Box::new(MarkRushApp::new(cc)))),
    )
}

fn load_app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/MarkRush-256.png"))
        .expect("embedded app icon PNG should be valid")
}

fn startup_window_size(settings: &AppSettings) -> [f32; 2] {
    saved_window_size(settings).unwrap_or(MIN_WINDOW_SIZE)
}

fn saved_window_size(settings: &AppSettings) -> Option<[f32; 2]> {
    let width = settings.window_width?;
    let height = settings.window_height?;

    (width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0).then_some([
        width.max(MIN_WINDOW_SIZE[0]),
        height.max(MIN_WINDOW_SIZE[1]),
    ])
}

fn first_launch_window_size(monitor_size: egui::Vec2) -> egui::Vec2 {
    if monitor_size.x < monitor_size.y {
        let width = (monitor_size.x * PORTRAIT_WINDOW_WIDTH_RATIO)
            .max(MIN_WINDOW_SIZE[0])
            .min(monitor_size.x.max(1.0));
        let height = (width * (MIN_WINDOW_SIZE[1] / MIN_WINDOW_SIZE[0]))
            .max(MIN_WINDOW_SIZE[1])
            .min(monitor_size.y.max(1.0));
        return egui::vec2(width, height);
    }

    egui::vec2(
        (monitor_size.x * FIRST_LAUNCH_WINDOW_SCREEN_SCALE).max(MIN_WINDOW_SIZE[0]),
        (monitor_size.y * FIRST_LAUNCH_WINDOW_SCREEN_SCALE).max(MIN_WINDOW_SIZE[1]),
    )
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct AppSettings {
    last_root_path: Option<PathBuf>,
    last_file_path: Option<PathBuf>,
    theme_name: String,
    word_wrap: bool,
    render_markdown: bool,
    document_font_family: String,
    document_font_size: f32,
    window_width: Option<f32>,
    window_height: Option<f32>,
    file_view_states: HashMap<PathBuf, FileViewState>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            last_root_path: None,
            last_file_path: None,
            theme_name: "EVA Dark".to_owned(),
            word_wrap: true,
            render_markdown: true,
            document_font_family: DEFAULT_DOCUMENT_FONT_FAMILY.to_owned(),
            document_font_size: DEFAULT_DOCUMENT_FONT_SIZE,
            window_width: None,
            window_height: None,
            file_view_states: HashMap::new(),
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct FileViewState {
    reader_line: usize,
}

#[derive(Clone, Copy)]
struct TopbarControlWidths {
    theme: f32,
    font: f32,
    font_size: f32,
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

struct DocumentFontChoice {
    label: &'static str,
    font_key: Option<&'static str>,
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
        Self::from_path(path, true)
    }

    fn from_path(path: PathBuf, force_dir: bool) -> Self {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| display_path(&path));
        let is_dir = force_dir || path.is_dir();

        Self {
            path,
            name,
            is_dir,
            loaded: !is_dir,
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
    CodeBlock { is_first: bool, is_last: bool },
    Rule,
    Table,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CodeFence {
    Backtick,
    Tilde,
}

#[derive(Clone)]
struct RenderLine {
    text: String,
    kind: RenderKind,
    copy_text: Option<String>,
}

impl RenderLine {
    fn new(text: impl Into<String>, kind: RenderKind) -> Self {
        Self {
            text: text.into(),
            kind,
            copy_text: None,
        }
    }

    fn code_block_line(
        text: impl Into<String>,
        is_first: bool,
        is_last: bool,
        copy_text: Option<String>,
    ) -> Self {
        Self {
            text: text.into(),
            kind: RenderKind::CodeBlock { is_first, is_last },
            copy_text,
        }
    }
}

#[derive(Clone)]
struct SearchMatch {
    line_index: usize,
}

#[derive(Clone, Copy)]
struct CodeCopyFeedback {
    line_index: usize,
    started: Instant,
}

struct TemporaryHierarchy {
    root: TreeEntry,
}

#[derive(Clone)]
enum PendingAction {
    OpenFile(PathBuf),
    OpenFileForEdit(PathBuf),
    OpenFileWithAnchor(PathBuf, String),
    SetRoot(PathBuf),
    NewUntitled,
    ClearFile,
}

enum TreeAction {
    OpenFile(PathBuf),
    ShowInBrowser(PathBuf),
    CopyPath(PathBuf),
    Rename(PathBuf),
}

#[derive(Clone, Copy)]
struct MiddleDragScroll {
    scroll_id: egui::Id,
    start_pos: egui::Pos2,
}

enum InitialWindowFit {
    NotNeeded,
    PendingResize,
    PendingSave { size: egui::Vec2, frames: u8 },
}

struct MarkRushApp {
    settings: AppSettings,
    settings_path: Option<PathBuf>,
    root: Option<TreeEntry>,
    temporary_hierarchies: Vec<TemporaryHierarchy>,
    current_file: Option<PathBuf>,
    untitled_document: bool,
    current_file_modified: Option<SystemTime>,
    document_text: String,
    saved_text: String,
    render_lines: Vec<RenderLine>,
    edit_mode: bool,
    dirty: bool,
    search_query: String,
    search_matches: Vec<SearchMatch>,
    current_match: usize,
    code_copy_feedback: Option<CodeCopyFeedback>,
    status: String,
    theme_index: usize,
    pending_action: Option<PendingAction>,
    show_save_prompt: bool,
    known_roots: HashSet<PathBuf>,
    pending_scroll_line: Option<usize>,
    focus_search: bool,
    middle_drag_scroll: Option<MiddleDragScroll>,
    initial_window_fit: InitialWindowFit,
    window_focused: bool,
    show_new_file_prompt: bool,
    new_file_name: String,
    rename_target: Option<PathBuf>,
    rename_name: String,
    print_preview: Option<PrintDocument>,
}

impl MarkRushApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_pixels_per_point(1.0);
        install_system_fonts(&cc.egui_ctx);

        let settings_path = settings_path();
        let settings = load_settings(settings_path.as_deref());
        let theme_index = theme_index_by_name(&settings.theme_name);
        let initial_window_fit = if saved_window_size(&settings).is_some() {
            InitialWindowFit::NotNeeded
        } else {
            InitialWindowFit::PendingResize
        };
        let window_focused = cc
            .egui_ctx
            .input(|input| input.viewport().focused.unwrap_or(true));

        let mut app = Self {
            settings,
            settings_path,
            root: None,
            temporary_hierarchies: Vec::new(),
            current_file: None,
            untitled_document: false,
            current_file_modified: None,
            document_text: String::new(),
            saved_text: String::new(),
            render_lines: Vec::new(),
            edit_mode: false,
            dirty: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match: 0,
            code_copy_feedback: None,
            status: "Ready".to_owned(),
            theme_index,
            pending_action: None,
            show_save_prompt: false,
            known_roots: HashSet::new(),
            pending_scroll_line: None,
            focus_search: false,
            middle_drag_scroll: None,
            initial_window_fit,
            window_focused,
            show_new_file_prompt: false,
            new_file_name: "untitled.md".to_owned(),
            rename_target: None,
            rename_name: String::new(),
            print_preview: None,
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
        self.update_window_title(ui.ctx());
        self.apply_initial_window_fit(ui.ctx());
        if !self.initial_window_fit_blocks_size_save() {
            self.remember_window_size(ui.ctx());
        }
        self.handle_shortcuts(ui);
        self.handle_focus_reload(ui.ctx());

        let full_rect = ui.max_rect();
        let theme = self.theme().clone();
        ui.painter().rect_filled(full_rect, 0, theme.window);

        let sidebar_width = 300.0;
        let divider_x = full_rect.min.x + sidebar_width;
        let sidebar_rect =
            egui::Rect::from_min_max(full_rect.min, egui::pos2(divider_x, full_rect.max.y));
        let workspace_rect =
            egui::Rect::from_min_max(egui::pos2(divider_x + 1.0, full_rect.min.y), full_rect.max);
        self.handle_drops(ui.ctx(), sidebar_rect, workspace_rect);

        ui.painter().line_segment(
            [
                egui::pos2(divider_x, full_rect.min.y),
                egui::pos2(divider_x, full_rect.max.y),
            ],
            egui::Stroke::new(1.0, theme.border),
        );

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(sidebar_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| self.draw_sidebar(ui),
        );

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(workspace_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| self.draw_workspace(ui, frame),
        );

        if self.show_save_prompt {
            self.draw_save_prompt(ui.ctx());
        }
        if self.show_new_file_prompt {
            self.draw_new_file_prompt(ui.ctx());
        }
        if self.rename_target.is_some() {
            self.draw_rename_prompt(ui.ctx());
        }
        if self.print_preview.is_some() {
            self.draw_print_preview(ui.ctx());
        }
    }

    fn draw_sidebar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0, theme.panel);
        let inner_rect = rect.shrink2(egui::vec2(12.0, 12.0));

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(inner_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
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

                        if egui::Button::new("New")
                            .fill(theme.button)
                            .ui(ui)
                            .on_hover_text("Create a Markdown document")
                            .clicked()
                        {
                            self.open_new_file_prompt();
                        }
                    });

                    ui.add_space(8.0);
                    let root_label = self
                        .root
                        .as_ref()
                        .map(|entry| display_path(&entry.path))
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

                    let mut pending_action = None;
                    let mut close_temporary_index = None;
                    let tree_output = egui::ScrollArea::vertical()
                        .id_salt("sidebar-tree-scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            apply_compact_tree_style(ui);
                            if let Some(root) = &mut self.root {
                                if let Some(action) =
                                    show_tree(ui, root, self.current_file.as_deref(), &theme)
                                {
                                    pending_action = Some(action);
                                }
                            } else if self.temporary_hierarchies.is_empty() {
                                ui.label(
                                    egui::RichText::new("Open a folder to browse Markdown files.")
                                        .color(theme.muted),
                                );
                            }

                            for (index, hierarchy) in
                                self.temporary_hierarchies.iter_mut().enumerate()
                            {
                                ui.add_space(10.0);
                                ui.separator();
                                ui.horizontal(|ui| {
                                    if egui::Button::new("Close")
                                        .fill(theme.button)
                                        .ui(ui)
                                        .clicked()
                                    {
                                        close_temporary_index = Some(index);
                                    }
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(display_path(&hierarchy.root.path))
                                                .color(theme.muted)
                                                .size(12.0),
                                        )
                                        .truncate(),
                                    );
                                });

                                if let Some(action) = show_tree(
                                    ui,
                                    &mut hierarchy.root,
                                    self.current_file.as_deref(),
                                    &theme,
                                ) {
                                    pending_action = Some(action);
                                }
                            }
                        });
                    self.apply_middle_mouse_scroll(ui, &tree_output, false, true);
                    if let Some(index) = close_temporary_index {
                        self.close_temporary_hierarchy(index);
                    }
                    if let Some(action) = pending_action {
                        self.handle_tree_action(ui.ctx(), action);
                    }
                });
            },
        );
    }

    fn draw_workspace(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let theme = self.theme().clone();
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0, theme.panel_alt);

        let chrome_gap = 8.0;
        let topbar_height = 112.0;
        let searchbar_height = 50.0;
        let statusbar_height = 30.0;
        let topbar_rect = egui::Rect::from_min_max(
            rect.min,
            egui::pos2(rect.max.x, (rect.min.y + topbar_height).min(rect.max.y)),
        );
        let searchbar_rect = egui::Rect::from_min_max(
            egui::pos2(rect.min.x, (topbar_rect.max.y + chrome_gap).min(rect.max.y)),
            egui::pos2(
                rect.max.x,
                (topbar_rect.max.y + chrome_gap + searchbar_height).min(rect.max.y),
            ),
        );
        let statusbar_rect = egui::Rect::from_min_max(
            egui::pos2(rect.min.x, (rect.max.y - statusbar_height).max(rect.min.y)),
            rect.max,
        );
        let document_min_y = (searchbar_rect.max.y + chrome_gap).min(statusbar_rect.min.y);
        let document_rect = egui::Rect::from_min_max(
            egui::pos2(rect.min.x, document_min_y),
            egui::pos2(rect.max.x, statusbar_rect.min.y.max(document_min_y)),
        );

        ui.painter().rect_filled(document_rect, 0, theme.window);
        ui.painter().line_segment(
            [
                egui::pos2(rect.min.x, statusbar_rect.min.y),
                egui::pos2(rect.max.x, statusbar_rect.min.y),
            ],
            egui::Stroke::new(1.0, theme.border),
        );

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(topbar_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| self.draw_topbar(ui),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(searchbar_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| self.draw_searchbar(ui),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(document_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.allocate_ui(document_rect.size(), |ui| {
                    if self.edit_mode {
                        self.draw_editor(ui);
                    } else {
                        self.draw_reader(ui);
                    }
                });
            },
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(statusbar_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| self.draw_statusbar(ui),
        );
    }

    fn draw_topbar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();

        egui::Frame::new()
            .fill(theme.panel_alt)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.vertical(|ui| {
                    let file_name = self.current_document_name();
                    ui.add_sized(
                        [ui.available_width(), 20.0],
                        egui::Label::new(
                            egui::RichText::new(file_name)
                                .strong()
                                .size(16.0)
                                .color(theme.text),
                        )
                        .truncate(),
                    );

                    ui.horizontal(|ui| {
                        let row_width = ui.available_width();
                        let spacing = ui.spacing().item_spacing.x;
                        let file_path = self.current_document_path_label();
                        ui.add_sized(
                            [
                                (row_width - TOPBAR_PATH_COPY_WIDTH - spacing).max(0.0),
                                18.0,
                            ],
                            egui::Label::new(
                                egui::RichText::new(file_path).size(12.0).color(theme.muted),
                            )
                            .truncate(),
                        );

                        let copy_response = ui
                            .add_enabled(
                                self.current_file.is_some(),
                                egui::Button::new(egui::RichText::new("📋").size(13.0))
                                    .fill(theme.button)
                                    .min_size(egui::vec2(TOPBAR_PATH_COPY_WIDTH, 22.0)),
                            )
                            .on_hover_text("Copy path");
                        if copy_response.clicked() {
                            if let Some(path) = &self.current_file {
                                ui.ctx().copy_text(display_path(path));
                                self.set_status("Copied file path");
                            }
                        }
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let row_width = ui.available_width();
                        let spacing = ui.spacing().item_spacing.x;
                        let left_width = topbar_left_controls_width(row_width);
                        let controls_width = (row_width - left_width - spacing).max(0.0);
                        let control_widths =
                            topbar_control_widths(controls_width, ui.spacing().item_spacing.x);

                        ui.allocate_ui_with_layout(
                            egui::vec2(left_width, TOPBAR_TOOLBAR_HEIGHT),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                let mut edit = self.edit_mode;
                                if ui.checkbox(&mut edit, "Edit").changed() {
                                    self.edit_mode = edit;
                                }

                                let mut wrap = self.settings.word_wrap;
                                if ui.checkbox(&mut wrap, "Wrap").changed() {
                                    self.settings.word_wrap = wrap;
                                    self.save_settings();
                                }

                                let mode_label = if self.settings.render_markdown {
                                    "Markdown"
                                } else {
                                    "Plain"
                                };
                                if egui::Button::new(mode_label)
                                    .fill(theme.button)
                                    .ui(ui)
                                    .clicked()
                                {
                                    self.settings.render_markdown = !self.settings.render_markdown;
                                    self.save_settings();
                                }
                            },
                        );

                        ui.allocate_ui_with_layout(
                            egui::vec2(controls_width, TOPBAR_TOOLBAR_HEIGHT),
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                self.draw_topbar_controls(ui, &theme, control_widths);
                            },
                        );
                    });
                });
            });
    }

    fn draw_topbar_controls(
        &mut self,
        ui: &mut egui::Ui,
        theme: &AppTheme,
        widths: TopbarControlWidths,
    ) {
        let mut selected_theme = self.theme().name.to_owned();
        egui::ComboBox::from_id_salt("theme-picker")
            .width(widths.theme)
            .selected_text(selected_theme.clone())
            .show_ui(ui, |ui| {
                for theme in themes() {
                    ui.selectable_value(&mut selected_theme, theme.name.to_owned(), theme.name);
                }
            });

        if selected_theme != self.theme().name {
            self.theme_index = theme_index_by_name(&selected_theme);
            self.settings.theme_name = selected_theme;
            self.save_settings();
        }

        let mut selected_font = self.document_font_choice().label.to_owned();
        egui::ComboBox::from_id_salt("document-font-picker")
            .width(widths.font)
            .selected_text(selected_font.clone())
            .show_ui(ui, |ui| {
                for font in document_fonts() {
                    ui.selectable_value(&mut selected_font, font.label.to_owned(), font.label);
                }
            });

        if selected_font != self.document_font_choice().label {
            self.settings.document_font_family = selected_font;
            self.save_settings();
        }

        let mut selected_font_size = self.document_font_size();
        egui::ComboBox::from_id_salt("document-font-size-picker")
            .width(widths.font_size)
            .selected_text(format!("{selected_font_size:.0}px"))
            .show_ui(ui, |ui| {
                for size in FONT_SIZE_OPTIONS {
                    ui.selectable_value(&mut selected_font_size, *size, format!("{size:.0}px"));
                }
            });

        if (selected_font_size - self.document_font_size()).abs() > f32::EPSILON {
            self.settings.document_font_size = selected_font_size;
            self.save_settings();
        }

        let can_save = self.has_open_document() && (self.dirty || self.edit_mode);
        if toolbar_icon_button(ui, "💾", "Save", can_save, theme).clicked() {
            self.save_current_file();
        }

        if toolbar_icon_button(ui, "🖨", "Print preview", self.current_file.is_some(), theme)
            .clicked()
        {
            self.open_print_preview();
        }

        if toolbar_icon_button(
            ui,
            "📁",
            "Show in file browser",
            self.current_file.is_some(),
            theme,
        )
        .clicked()
        {
            if let Some(path) = &self.current_file {
                if let Err(error) = show_in_file_browser(path) {
                    self.set_status(format!("Could not show file: {error}"));
                } else {
                    self.set_status("Opened file in system file browser");
                }
            }
        }
    }

    fn draw_searchbar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();

        egui::Frame::new()
            .fill(theme.panel_alt)
            .inner_margin(egui::Margin::symmetric(16, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let search_id = ui.make_persistent_id("search-box");
                    if self.focus_search {
                        ui.memory_mut(|memory| memory.request_focus(search_id));
                        self.focus_search = false;
                    }

                    let response = ui.add_enabled(
                        self.has_open_document(),
                        egui::TextEdit::singleline(&mut self.search_query)
                            .id(search_id)
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

    fn draw_statusbar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();
        egui::Frame::new()
            .fill(theme.panel)
            .inner_margin(egui::Margin::symmetric(12, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(APP_BUILD_LABEL).color(theme.muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} lines, {} chars",
                                document_line_count(&self.document_text),
                                self.document_text.chars().count()
                            ))
                            .color(theme.muted),
                        );
                    });
                });
            });
    }

    fn draw_reader(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();

        if !self.has_open_document() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Open a Markdown file")
                        .color(theme.muted)
                        .size(18.0),
                );
            });
            return;
        }

        if !self.settings.render_markdown {
            self.draw_plain_reader(ui);
            return;
        }

        let current_match_line = self
            .search_matches
            .get(self.current_match)
            .map(|search_match| search_match.line_index);

        let pending_scroll_line = self.pending_scroll_line.take();
        let current_match_line = current_match_line.or(pending_scroll_line);
        let font_size = self.document_font_size();
        let font_family = self.document_font_family();
        let row_height = reader_row_height(font_size);
        let mut clicked_link = None;
        let mut copied_code_line = None;
        if self
            .code_copy_feedback
            .is_some_and(|feedback| feedback.started.elapsed() >= CODE_COPY_FEEDBACK_DURATION)
        {
            self.code_copy_feedback = None;
        }

        let file_scroll_id = egui::Id::new("reader-scroll").with(
            self.current_file
                .as_ref()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
        );
        let mut scroll = if self.settings.word_wrap {
            egui::ScrollArea::vertical()
        } else {
            egui::ScrollArea::both()
        }
        .id_salt(file_scroll_id)
        .content_margin(egui::Margin::symmetric(18, 12))
        .auto_shrink([false, false]);
        let wrap = self.settings.word_wrap;
        let has_code_blocks = self
            .render_lines
            .iter()
            .any(|line| matches!(line.kind, RenderKind::CodeBlock { .. }));
        let (output, reader_line) = if wrap || has_code_blocks {
            let reader_width = if wrap {
                reader_estimate_width(ui.available_width())
            } else {
                UNWRAPPED_DOCUMENT_WIDTH
            };
            let estimated_heights =
                estimate_reader_heights(&self.render_lines, font_size, reader_width, wrap);
            let total_height = total_reader_height(&estimated_heights);
            if let Some(offset) =
                pending_scroll_line.map(|line| reader_offset_for_line(&estimated_heights, line))
            {
                scroll = scroll.vertical_scroll_offset(offset);
            }

            let output = scroll.show_viewport(ui, |ui, viewport| {
                if !wrap {
                    ui.set_min_width(UNWRAPPED_DOCUMENT_WIDTH);
                }

                let overscan = reader_scroll_overscan(viewport.height());
                let (start, end, top_space, bottom_space) =
                    reader_virtual_range(&estimated_heights, viewport, overscan, total_height);

                if top_space > 0.0 {
                    ui.add_space(top_space);
                }

                for index in start..end {
                    if let Some(line) = self.render_lines.get(index) {
                        let is_match = current_match_line == Some(index);
                        let output = draw_render_line(
                            ui,
                            line,
                            &theme,
                            is_match,
                            wrap,
                            font_size,
                            &font_family,
                            code_copy_feedback_alpha(self.code_copy_feedback, index),
                        );
                        if let Some(target) = output.clicked_link {
                            clicked_link = Some(target);
                        }
                        if output.copied_code {
                            copied_code_line = Some(index);
                        }
                    }
                }

                if bottom_space > 0.0 {
                    ui.add_space(bottom_space);
                }
            });
            let reader_line =
                line_index_at_reader_offset(&estimated_heights, output.state.offset.y);
            (output, reader_line)
        } else {
            if let Some(offset) = pending_scroll_line.map(|line| line as f32 * row_height) {
                scroll = scroll.vertical_scroll_offset(offset);
            }

            let output =
                scroll.show_rows(ui, row_height, self.render_lines.len(), |ui, row_range| {
                    ui.set_min_width(UNWRAPPED_DOCUMENT_WIDTH);

                    for index in row_range {
                        if let Some(line) = self.render_lines.get(index) {
                            let is_match = current_match_line == Some(index);
                            let output = draw_render_line(
                                ui,
                                line,
                                &theme,
                                is_match,
                                wrap,
                                font_size,
                                &font_family,
                                code_copy_feedback_alpha(self.code_copy_feedback, index),
                            );
                            if let Some(target) = output.clicked_link {
                                clicked_link = Some(target);
                            }
                            if output.copied_code {
                                copied_code_line = Some(index);
                            }
                        }
                    }
                });
            let reader_line = (output.state.offset.y / row_height).floor().max(0.0) as usize;
            (output, reader_line)
        };
        self.apply_middle_mouse_scroll(ui, &output, !self.settings.word_wrap, true);

        if let Some(path) = self.current_file.clone() {
            let state = FileViewState { reader_line };

            if self.settings.file_view_states.get(&path) != Some(&state) {
                self.settings.file_view_states.insert(path, state);
                self.trim_stored_file_view_states();
                self.save_settings();
            }
        }

        if let Some(target) = clicked_link {
            self.follow_link(&target);
        }

        if let Some(line_index) = copied_code_line {
            self.code_copy_feedback = Some(CodeCopyFeedback {
                line_index,
                started: Instant::now(),
            });
        }
        if self.code_copy_feedback.is_some() {
            ui.ctx().request_repaint_after(ANIMATION_FRAME_INTERVAL);
        }
    }

    fn draw_plain_reader(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();
        let font_size = self.document_font_size();
        let font_family = self.document_font_family();

        let file_scroll_id = egui::Id::new("plain-reader-scroll").with(
            self.current_file
                .as_ref()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
        );
        let mut scroll = if self.settings.word_wrap {
            egui::ScrollArea::vertical()
        } else {
            egui::ScrollArea::both()
        }
        .id_salt(file_scroll_id)
        .content_margin(egui::Margin::symmetric(18, 12))
        .auto_shrink([false, false]);

        if let Some(line) = self.pending_scroll_line.take() {
            scroll = scroll.vertical_scroll_offset(line as f32 * reader_row_height(font_size));
        }

        let output = scroll.show(ui, |ui| {
            if !self.settings.word_wrap {
                ui.set_min_width(UNWRAPPED_DOCUMENT_WIDTH);
            }

            let label = egui::Label::new(
                egui::RichText::new(&self.document_text)
                    .font(egui::FontId::new(font_size, font_family))
                    .color(theme.text),
            );
            if self.settings.word_wrap {
                ui.add(label.wrap());
            } else {
                ui.add(label);
            }
        });
        self.apply_middle_mouse_scroll(ui, &output, !self.settings.word_wrap, true);
    }

    fn draw_editor(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme().clone();

        if !self.has_open_document() {
            ui.centered_and_justified(|ui| {
                ui.label("Open a Markdown file to edit");
            });
            return;
        }

        let desired_width = if self.settings.word_wrap {
            ui.available_width()
        } else {
            UNWRAPPED_DOCUMENT_WIDTH
        };
        let font_size = self.document_font_size();
        let font_family = self.document_editor_font_family();

        let edit = egui::TextEdit::multiline(&mut self.document_text)
            .id(egui::Id::new("editor").with(
                self.current_file
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|| "untitled-document".to_owned()),
            ))
            .desired_width(desired_width)
            .font(egui::FontId::new(font_size, font_family))
            .text_color(theme.text)
            .code_editor()
            .lock_focus(true);

        let editor_scroll_id = egui::Id::new("editor-scroll").with(
            self.current_file
                .as_ref()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled-document".to_owned()),
        );

        let response = if self.settings.word_wrap {
            let output = egui::ScrollArea::vertical()
                .id_salt(editor_scroll_id.with("wrap"))
                .content_margin(egui::Margin::same(14))
                .auto_shrink([false, false])
                .show(ui, |ui| ui.add_sized(ui.available_size(), edit));
            self.apply_selection_drag_scroll(ui, &output, false, true);
            self.apply_middle_mouse_scroll(ui, &output, false, true);
            output.inner
        } else {
            let output = egui::ScrollArea::both()
                .id_salt(editor_scroll_id.with("both"))
                .content_margin(egui::Margin::same(14))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_sized(egui::vec2(desired_width, ui.available_height()), edit)
                });
            self.apply_selection_drag_scroll(ui, &output, true, true);
            self.apply_middle_mouse_scroll(ui, &output, true, true);
            output.inner
        };

        if response.changed() {
            self.dirty = self.document_text != self.saved_text;
            self.render_lines = parse_markdown_lines(&self.document_text);
            self.rebuild_search();
        }
    }

    fn apply_middle_mouse_scroll<R>(
        &mut self,
        ui: &mut egui::Ui,
        output: &egui::containers::scroll_area::ScrollAreaOutput<R>,
        allow_x: bool,
        allow_y: bool,
    ) {
        let (middle_down, pointer_pos) = ui.input(|input| {
            (
                input.pointer.button_down(egui::PointerButton::Middle),
                input.pointer.latest_pos(),
            )
        });

        if !middle_down {
            self.middle_drag_scroll = None;
            return;
        }

        let Some(pointer_pos) = pointer_pos else {
            return;
        };

        let is_active = self
            .middle_drag_scroll
            .as_ref()
            .is_some_and(|drag| drag.scroll_id == output.id);

        if !is_active {
            if self.middle_drag_scroll.is_none()
                && output.inner_rect.expand(18.0).contains(pointer_pos)
            {
                self.middle_drag_scroll = Some(MiddleDragScroll {
                    scroll_id: output.id,
                    start_pos: pointer_pos,
                });
                ui.ctx().set_cursor_icon(egui::CursorIcon::AllScroll);
                ui.ctx().request_repaint_after(ANIMATION_FRAME_INTERVAL);
            }
            return;
        }

        let Some(drag) = self.middle_drag_scroll.as_mut() else {
            return;
        };

        let delta = pointer_pos - drag.start_pos;
        ui.ctx().set_cursor_icon(egui::CursorIcon::AllScroll);

        let mut state = output.state;
        let max_offset = output.content_size - output.inner_rect.size();
        let frame_scale = ui.input(|input| (input.stable_dt / (1.0 / 60.0)).clamp(0.25, 3.0));
        let mut changed = false;

        if allow_x {
            let scroll_delta = middle_scroll_step(delta.x) * frame_scale;
            if scroll_delta != 0.0 {
                state.offset.x = (state.offset.x + scroll_delta).clamp(0.0, max_offset.x.max(0.0));
                changed = true;
            }
        }

        if allow_y {
            let scroll_delta = middle_scroll_step(delta.y) * frame_scale;
            if scroll_delta != 0.0 {
                state.offset.y = (state.offset.y + scroll_delta).clamp(0.0, max_offset.y.max(0.0));
                changed = true;
            }
        }

        if changed {
            state.store(ui.ctx(), output.id);
        }
        ui.ctx().request_repaint_after(ANIMATION_FRAME_INTERVAL);
    }

    fn apply_selection_drag_scroll<R>(
        &mut self,
        ui: &mut egui::Ui,
        output: &egui::containers::scroll_area::ScrollAreaOutput<R>,
        allow_x: bool,
        allow_y: bool,
    ) {
        let (primary_down, pointer_pos) = ui.input(|input| {
            (
                input.pointer.button_down(egui::PointerButton::Primary),
                input.pointer.latest_pos(),
            )
        });

        let Some(pointer_pos) = pointer_pos else {
            return;
        };

        if !primary_down {
            return;
        }

        let mut state = output.state;
        let mut changed = false;
        let max_offset = output.content_size - output.inner_rect.size();
        let frame_scale = ui.input(|input| (input.stable_dt / (1.0 / 60.0)).clamp(0.25, 3.0));
        let dead_zone = SELECTION_SCROLL_DEAD_ZONE;
        let max_speed = SELECTION_SCROLL_MAX_SPEED;

        let vertical_margin = SELECTION_SCROLL_ZONE_MULTIPLIER * dead_zone;
        let horizontal_margin = SELECTION_SCROLL_ZONE_MULTIPLIER * dead_zone;
        let edge_rect = output.inner_rect.expand(horizontal_margin.max(vertical_margin));

        if !edge_rect.contains(pointer_pos) {
            return;
        }

        if allow_y {
            let mut delta_y = 0.0;
            if pointer_pos.y < output.inner_rect.min.y {
                delta_y = -(selection_scroll_speed(
                    output.inner_rect.min.y - pointer_pos.y,
                    dead_zone,
                    max_speed,
                ) * frame_scale);
            } else if pointer_pos.y > output.inner_rect.max.y {
                delta_y = selection_scroll_speed(
                    pointer_pos.y - output.inner_rect.max.y,
                    dead_zone,
                    max_speed,
                ) * frame_scale;
            }

            if delta_y != 0.0 {
                state.offset.y = (state.offset.y + delta_y).clamp(0.0, max_offset.y.max(0.0));
                changed = true;
            }
        }

        if allow_x {
            let mut delta_x = 0.0;
            if pointer_pos.x < output.inner_rect.min.x {
                delta_x = -(selection_scroll_speed(
                    output.inner_rect.min.x - pointer_pos.x,
                    dead_zone,
                    max_speed,
                ) * frame_scale);
            } else if pointer_pos.x > output.inner_rect.max.x {
                delta_x = selection_scroll_speed(
                    pointer_pos.x - output.inner_rect.max.x,
                    dead_zone,
                    max_speed,
                ) * frame_scale;
            }

            if delta_x != 0.0 {
                state.offset.x = (state.offset.x + delta_x).clamp(0.0, max_offset.x.max(0.0));
                changed = true;
            }
        }

        if changed {
            state.store(ui.ctx(), output.id);
            ui.ctx().request_repaint_after(ANIMATION_FRAME_INTERVAL);
        }
    }

    fn draw_save_prompt(&mut self, ctx: &egui::Context) {
        let theme = self.theme().clone();
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                let file_name = self
                    .current_file
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or(if self.untitled_document {
                        "Untitled.md"
                    } else {
                        "this file"
                    });
                ui.label(format!("Save changes to {file_name}?"));
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

    fn draw_new_file_prompt(&mut self, ctx: &egui::Context) {
        let theme = self.theme().clone();
        egui::Window::new("New file")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("File name:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.new_file_name)
                        .desired_width(280.0)
                        .hint_text("filename.md"),
                );
                if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    self.create_new_file_from_prompt();
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if egui::Button::new("Create")
                        .fill(theme.button)
                        .ui(ui)
                        .clicked()
                    {
                        self.create_new_file_from_prompt();
                    }

                    if egui::Button::new("Cancel")
                        .fill(theme.button)
                        .ui(ui)
                        .clicked()
                    {
                        self.show_new_file_prompt = false;
                    }
                });
            });
    }

    fn draw_rename_prompt(&mut self, ctx: &egui::Context) {
        let theme = self.theme().clone();
        egui::Window::new("Rename")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("New name:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.rename_name)
                        .desired_width(280.0)
                        .hint_text("filename.md"),
                );
                if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    self.rename_item_from_prompt();
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if egui::Button::new("Rename")
                        .fill(theme.button)
                        .ui(ui)
                        .clicked()
                    {
                        self.rename_item_from_prompt();
                    }

                    if egui::Button::new("Cancel")
                        .fill(theme.button)
                        .ui(ui)
                        .clicked()
                    {
                        self.rename_target = None;
                        self.rename_name.clear();
                    }
                });
            });
    }

    fn draw_print_preview(&mut self, ctx: &egui::Context) {
        let theme = self.theme().clone();
        let mut should_print = false;
        let mut should_close = false;

        {
            let Some(preview) = self.print_preview.as_ref() else {
                return;
            };
            let page_count = preview.pages.len();

            egui::Window::new("Print preview")
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(760.0, 620.0))
                .min_size(egui::vec2(520.0, 420.0))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&preview.title)
                                    .strong()
                                    .color(theme.text),
                            )
                            .truncate(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!("{page_count} page(s)"))
                                    .color(theme.muted),
                            );
                        });
                    });

                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .id_salt("print-preview-pages")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (index, page) in preview.pages.iter().enumerate() {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Page {} of {page_count}",
                                        index + 1
                                    ))
                                    .color(theme.muted)
                                    .size(12.0),
                                );
                                egui::Frame::new()
                                    .fill(egui::Color32::WHITE)
                                    .stroke(egui::Stroke::new(1.0, theme.border))
                                    .inner_margin(egui::Margin::symmetric(22, 24))
                                    .show(ui, |ui| {
                                        ui.set_min_width(610.0);
                                        ui.set_min_height(790.0);
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        for line in &page.lines {
                                            let visible_line =
                                                if line.is_empty() { " " } else { line };
                                            ui.label(
                                                egui::RichText::new(visible_line)
                                                    .monospace()
                                                    .size(11.0)
                                                    .color(egui::Color32::BLACK),
                                            );
                                        }
                                    });
                                ui.add_space(14.0);
                            }
                        });

                    ui.separator();
                    ui.horizontal(|ui| {
                        if egui::Button::new("Print")
                            .fill(theme.button)
                            .ui(ui)
                            .clicked()
                        {
                            should_print = true;
                        }

                        if egui::Button::new("Close")
                            .fill(theme.button)
                            .ui(ui)
                            .clicked()
                        {
                            should_close = true;
                        }
                    });
                });
        }

        if should_print {
            self.print_preview_document();
        }
        if should_close {
            self.print_preview = None;
        }
    }

    fn handle_shortcuts(&mut self, ui: &mut egui::Ui) {
        ui.input(|input| {
            if input.key_pressed(egui::Key::S) && input.modifiers.command {
                self.save_current_file();
            }

            if input.key_pressed(egui::Key::P) && input.modifiers.command {
                self.open_print_preview();
            }

            if self.has_open_document()
                && input.modifiers.command
                && (input.key_pressed(egui::Key::V) || input.key_pressed(egui::Key::X))
            {
                self.edit_mode = true;
            }

            if input.key_pressed(egui::Key::F) && input.modifiers.command {
                self.focus_search = true;
            }

            if input.key_pressed(egui::Key::F5) {
                self.refresh_tree();
            }

            if input.key_pressed(egui::Key::Escape) {
                self.middle_drag_scroll = None;
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

    fn handle_focus_reload(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|input| input.viewport().focused.unwrap_or(self.window_focused));
        let regained_focus = focused && !self.window_focused;
        self.window_focused = focused;

        if regained_focus {
            self.refresh_tree_silent();
            self.reload_current_file_if_changed_on_disk();
        }
    }

    fn handle_drops(
        &mut self,
        ctx: &egui::Context,
        sidebar_rect: egui::Rect,
        workspace_rect: egui::Rect,
    ) {
        let (dropped_paths, dropped_in_workspace) = ctx.input(|input| {
            let dropped_paths = input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>();
            let Some(pointer_pos) = input.pointer.hover_pos() else {
                return (dropped_paths, false);
            };
            let accepted_drop_zone =
                sidebar_rect.contains(pointer_pos) || workspace_rect.contains(pointer_pos);
            let dropped_in_workspace =
                workspace_rect.contains(pointer_pos) && !sidebar_rect.contains(pointer_pos);

            (
                accepted_drop_zone
                    .then_some(dropped_paths)
                    .unwrap_or_default(),
                dropped_in_workspace,
            )
        });

        if dropped_paths.is_empty() {
            return;
        }

        let mut added = 0;
        let mut file_to_open = None;
        for path in dropped_paths {
            if dropped_in_workspace && file_to_open.is_none() && is_markdown_file(&path) {
                file_to_open = Some(path.clone());
            }
            if self.add_temporary_hierarchy(path) {
                added += 1;
            }
        }

        if let Some(path) = file_to_open {
            self.request_open_file(path);
            if added > 1 {
                self.set_status(format!(
                    "Added {added} temporary hierarchies and opened file"
                ));
            } else {
                self.set_status("Added temporary hierarchy and opened file");
            }
        } else if added == 1 {
            self.set_status("Added temporary hierarchy");
        } else if added > 1 {
            self.set_status(format!("Added {added} temporary hierarchies"));
        }
    }

    fn add_temporary_hierarchy(&mut self, path: PathBuf) -> bool {
        let path = absolute_path(&path);
        if !path.is_dir() && !path.is_file() {
            return false;
        }
        if self
            .temporary_hierarchies
            .iter()
            .any(|hierarchy| hierarchy.root.path == path)
        {
            return false;
        }

        self.known_roots.insert(path.clone());
        self.temporary_hierarchies.push(TemporaryHierarchy {
            root: TreeEntry::from_path(path, false),
        });
        true
    }

    fn close_temporary_hierarchy(&mut self, index: usize) {
        if index < self.temporary_hierarchies.len() {
            let hierarchy = self.temporary_hierarchies.remove(index);
            self.known_roots.remove(&hierarchy.root.path);
            self.set_status("Closed temporary hierarchy");
        }
    }

    fn open_folder_dialog(&mut self) {
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            self.request_action(PendingAction::SetRoot(folder));
        }
    }

    fn open_new_file_prompt(&mut self) {
        let Some(parent) = self.new_file_parent() else {
            self.request_action(PendingAction::NewUntitled);
            return;
        };

        self.new_file_name = unique_markdown_file_name(&parent, "untitled.md");
        self.show_new_file_prompt = true;
    }

    fn create_new_file_from_prompt(&mut self) {
        let Some(parent) = self.new_file_parent() else {
            self.request_action(PendingAction::NewUntitled);
            self.show_new_file_prompt = false;
            return;
        };
        let Some(file_name) = normalized_prompt_file_name(&self.new_file_name, true) else {
            self.set_status("Enter a valid file name");
            return;
        };

        let target = parent.join(file_name);
        if target.exists() {
            self.set_status("A file with that name already exists");
            return;
        }

        match fs::write(&target, b"") {
            Ok(()) => {
                self.show_new_file_prompt = false;
                self.refresh_tree();
                self.request_action(PendingAction::OpenFileForEdit(target));
            }
            Err(error) => self.set_status(format!("Could not create file: {error}")),
        }
    }

    fn open_rename_prompt(&mut self, path: PathBuf) {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            self.set_status("Could not rename this item");
            return;
        };

        self.rename_name = name.to_owned();
        self.rename_target = Some(path);
    }

    fn rename_item_from_prompt(&mut self) {
        let Some(source) = self.rename_target.clone() else {
            return;
        };
        let Some(parent) = source.parent() else {
            self.set_status("Could not rename this item");
            return;
        };
        let Some(file_name) = normalized_prompt_file_name(&self.rename_name, false) else {
            self.set_status("Enter a valid name");
            return;
        };

        let target = parent.join(file_name);
        if source == target {
            self.rename_target = None;
            self.rename_name.clear();
            return;
        }
        if target.exists() {
            self.set_status("An item with that name already exists");
            return;
        }

        match fs::rename(&source, &target) {
            Ok(()) => {
                self.after_item_renamed(&source, &target);
                self.rename_target = None;
                self.rename_name.clear();
                self.set_status(format!("Renamed to {}", display_path(&target)));
            }
            Err(error) => self.set_status(format!("Rename failed: {error}")),
        }
    }

    fn after_item_renamed(&mut self, source: &Path, target: &Path) {
        if let Some(current_file) = self.current_file.clone() {
            if current_file == source {
                self.current_file = Some(target.to_path_buf());
                self.current_file_modified = file_modified_time(target).ok();
                self.settings.last_file_path = Some(target.to_path_buf());
                self.move_file_view_state(source, target);
            } else if let Ok(relative) = current_file.strip_prefix(source) {
                let renamed_file = target.join(relative);
                self.current_file = Some(renamed_file.clone());
                self.current_file_modified = file_modified_time(&renamed_file).ok();
                self.settings.last_file_path = Some(renamed_file);
            }
        }

        if self.root.as_ref().is_some_and(|root| root.path == source) {
            self.root = Some(TreeEntry::root(target.to_path_buf()));
            self.settings.last_root_path = Some(target.to_path_buf());
            self.known_roots.remove(source);
            self.known_roots.insert(target.to_path_buf());
        } else {
            self.refresh_tree();
        }

        for hierarchy in &mut self.temporary_hierarchies {
            if hierarchy.root.path == source {
                hierarchy.root = TreeEntry::from_path(target.to_path_buf(), false);
            } else if let Ok(relative) = hierarchy.root.path.strip_prefix(source) {
                hierarchy.root = TreeEntry::from_path(target.join(relative), false);
            }
        }

        self.save_settings();
    }

    fn move_file_view_state(&mut self, source: &Path, target: &Path) {
        if let Some(state) = self.settings.file_view_states.remove(source) {
            self.settings
                .file_view_states
                .insert(target.to_path_buf(), state);
        }
    }

    fn current_file_parent(&self) -> Option<PathBuf> {
        self.current_file
            .as_ref()
            .and_then(|path| path.parent())
            .map(Path::to_path_buf)
    }

    fn new_file_parent(&self) -> Option<PathBuf> {
        self.current_file_parent()
            .or_else(|| self.root.as_ref().map(|root| root.path.clone()))
    }

    fn handle_tree_action(&mut self, ctx: &egui::Context, action: TreeAction) {
        match action {
            TreeAction::OpenFile(path) => self.request_open_file(path),
            TreeAction::ShowInBrowser(path) => match show_in_file_browser(&path) {
                Ok(()) => self.set_status("Opened item in system file browser"),
                Err(error) => self.set_status(format!("Could not show item: {error}")),
            },
            TreeAction::CopyPath(path) => {
                ctx.copy_text(display_path(&path));
                self.set_status("Copied item path");
            }
            TreeAction::Rename(path) => self.open_rename_prompt(path),
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
            PendingAction::OpenFileForEdit(path) => self.load_file_for_edit(path),
            PendingAction::OpenFileWithAnchor(path, fragment) => {
                self.load_file(path);
                self.jump_to_anchor(&fragment);
            }
            PendingAction::SetRoot(path) => {
                self.set_root(path);
                self.apply_action(PendingAction::ClearFile);
            }
            PendingAction::NewUntitled => self.start_untitled_document(),
            PendingAction::ClearFile => self.clear_file(),
        }
    }

    fn set_root(&mut self, path: PathBuf) {
        let root_path = absolute_path(&path);
        self.root = Some(TreeEntry::root(root_path.clone()));
        self.known_roots.insert(root_path.clone());
        self.settings.last_root_path = Some(root_path.clone());
        self.save_settings();
        self.set_status(format!("Opened folder {}", display_path(&root_path)));
    }

    fn refresh_tree(&mut self) {
        if self.refresh_tree_silent() {
            self.set_status("Folder tree refreshed");
        }
    }

    fn refresh_tree_silent(&mut self) -> bool {
        let mut refreshed = false;
        if let Some(root) = self.root.as_ref().map(|entry| entry.path.clone()) {
            self.root = Some(TreeEntry::root(root));
            refreshed = true;
        }
        for hierarchy in &mut self.temporary_hierarchies {
            let path = hierarchy.root.path.clone();
            hierarchy.root = TreeEntry::from_path(path, false);
            refreshed = true;
        }
        refreshed
    }

    fn reload_current_file_if_changed_on_disk(&mut self) {
        let Some(path) = self.current_file.clone() else {
            return;
        };
        let Ok(modified) = file_modified_time(&path) else {
            return;
        };

        if self.current_file_modified == Some(modified) {
            return;
        }

        self.current_file_modified = Some(modified);
        if self.dirty {
            self.set_status("File changed on disk; unsaved edits kept");
            return;
        }

        self.load_file_with_status(path, "Auto-refreshed");
    }

    fn load_file(&mut self, path: PathBuf) {
        self.load_file_with_status(path, "Loaded");
    }

    fn load_file_for_edit(&mut self, path: PathBuf) {
        if self.load_file_with_status(path, "Created") {
            self.edit_mode = true;
        }
    }

    fn load_file_with_status(&mut self, path: PathBuf, verb: &str) -> bool {
        let full_path = absolute_path(&path);
        match fs::read(&full_path) {
            Ok(bytes) => {
                self.document_text = String::from_utf8_lossy(&bytes).to_string();
                self.saved_text = self.document_text.clone();
                self.render_lines = parse_markdown_lines(&self.document_text);
                self.current_file = Some(full_path.clone());
                self.untitled_document = false;
                self.current_file_modified = file_modified_time(&full_path).ok();
                self.dirty = false;
                self.edit_mode = false;
                self.code_copy_feedback = None;
                self.settings.last_file_path = Some(full_path.clone());
                self.pending_scroll_line = self
                    .settings
                    .file_view_states
                    .get(&full_path)
                    .map(|state| state.reader_line);
                self.save_settings();
                self.rebuild_search();
                self.set_status(format!("{verb} {}", display_path(&full_path)));
                true
            }
            Err(error) => {
                self.set_status(format!("Could not load file: {error}"));
                false
            }
        }
    }

    fn start_untitled_document(&mut self) {
        self.current_file = None;
        self.untitled_document = true;
        self.current_file_modified = None;
        self.document_text.clear();
        self.saved_text.clear();
        self.render_lines.clear();
        self.dirty = false;
        self.edit_mode = true;
        self.code_copy_feedback = None;
        self.pending_scroll_line = None;
        self.search_matches.clear();
        self.current_match = 0;
        self.settings.last_file_path = None;
        self.save_settings();
        self.set_status("New untitled Markdown document");
    }

    fn clear_file(&mut self) {
        self.current_file = None;
        self.untitled_document = false;
        self.current_file_modified = None;
        self.document_text.clear();
        self.saved_text.clear();
        self.render_lines.clear();
        self.dirty = false;
        self.edit_mode = false;
        self.code_copy_feedback = None;
        self.search_matches.clear();
        self.current_match = 0;
        self.settings.last_file_path = None;
        self.save_settings();
    }

    fn save_current_file(&mut self) -> bool {
        let path = match self.current_file.clone() {
            Some(path) => path,
            None if self.untitled_document => {
                let Some(path) = self.pick_untitled_save_path() else {
                    self.set_status("Save cancelled");
                    return false;
                };
                path
            }
            None => {
                self.set_status("No file to save");
                return false;
            }
        };

        match fs::write(&path, self.document_text.as_bytes()) {
            Ok(()) => {
                let path = absolute_path(&path);
                self.current_file = Some(path.clone());
                self.untitled_document = false;
                self.saved_text = self.document_text.clone();
                self.current_file_modified = file_modified_time(&path).ok();
                self.dirty = false;
                self.render_lines = parse_markdown_lines(&self.document_text);
                self.settings.last_file_path = Some(path.clone());
                if let Some(parent) = path.parent() {
                    if self.root.is_none() {
                        self.set_root(parent.to_path_buf());
                    } else {
                        self.refresh_tree_silent();
                    }
                }
                self.save_settings();
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

    fn pick_untitled_save_path(&self) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Markdown", MARKDOWN_EXTENSIONS)
            .set_file_name("untitled.md");

        if let Some(parent) = self.new_file_parent() {
            dialog = dialog.set_directory(parent);
        }

        dialog.save_file().map(with_default_markdown_extension)
    }

    fn open_print_preview(&mut self) {
        let Some(path) = self.current_file.as_ref() else {
            self.set_status("Select a file before printing");
            return;
        };

        let title = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("MarkRush document");

        let document = build_print_document(title, &self.document_text);
        let page_count = document.pages.len();
        self.print_preview = Some(document);
        self.set_status(format!("Prepared print preview ({page_count} page(s))"));
    }

    fn print_preview_document(&mut self) {
        let Some(document) = self.print_preview.as_ref() else {
            self.set_status("Open print preview before printing");
            return;
        };

        match print_document(document) {
            Ok(PrintOutcome::Printed) => self.set_status("Sent document to printer"),
            Ok(PrintOutcome::Cancelled) => self.set_status("Print cancelled"),
            Err(error) => self.set_status(format!("Print failed: {error}")),
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
        self.pending_scroll_line = self
            .search_matches
            .get(self.current_match)
            .map(|search_match| search_match.line_index);
    }

    fn search_status(&self) -> String {
        if !self.has_open_document() {
            "No file".to_owned()
        } else if self.search_query.trim().is_empty() {
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

    fn has_open_document(&self) -> bool {
        self.current_file.is_some() || self.untitled_document
    }

    fn current_document_name(&self) -> String {
        self.current_file
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                if self.untitled_document {
                    "Untitled.md".to_owned()
                } else {
                    "No file selected".to_owned()
                }
            })
    }

    fn current_document_path_label(&self) -> String {
        self.current_file
            .as_ref()
            .map(|path| display_path(path))
            .unwrap_or_else(|| {
                if self.untitled_document {
                    "Unsaved document".to_owned()
                } else {
                    String::new()
                }
            })
    }

    fn document_font_size(&self) -> f32 {
        sanitize_document_font_size(self.settings.document_font_size)
    }

    fn document_font_choice(&self) -> &'static DocumentFontChoice {
        document_font_by_label(&self.settings.document_font_family)
    }

    fn document_font_family(&self) -> egui::FontFamily {
        match self.document_font_choice().font_key {
            Some(font_key) => egui::FontFamily::Name(font_key.into()),
            None => egui::FontFamily::Proportional,
        }
    }

    fn document_editor_font_family(&self) -> egui::FontFamily {
        match self.document_font_choice().font_key {
            Some(font_key) => egui::FontFamily::Name(font_key.into()),
            None => egui::FontFamily::Monospace,
        }
    }

    fn update_window_title(&self, ctx: &egui::Context) {
        let marker = if self.dirty { "*" } else { "" };
        let file_name = self
            .current_file
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("MarkRush");
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{marker}{file_name} - MarkRush"
        )));
    }

    fn trim_stored_file_view_states(&mut self) {
        if self.settings.file_view_states.len() <= MAX_STORED_FILE_VIEW_STATES {
            return;
        }

        let mut paths: Vec<PathBuf> = self.settings.file_view_states.keys().cloned().collect();
        paths.sort_by_key(|path| {
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });

        let excess = self
            .settings
            .file_view_states
            .len()
            .saturating_sub(MAX_STORED_FILE_VIEW_STATES);
        for path in paths.into_iter().take(excess) {
            self.settings.file_view_states.remove(&path);
        }
    }

    fn theme(&self) -> &'static AppTheme {
        &themes()[self.theme_index]
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let theme = self.theme();
        let mut visuals = if theme.name == "Pearl White" {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        visuals.window_fill = theme.panel_alt;
        visuals.panel_fill = theme.panel_alt;
        visuals.extreme_bg_color = theme.input;
        visuals.faint_bg_color = theme.panel;
        visuals.code_bg_color = theme.code;
        visuals.widgets.noninteractive.bg_fill = theme.panel_alt;
        visuals.widgets.noninteractive.bg_stroke.color = theme.border;
        visuals.widgets.noninteractive.fg_stroke.color = theme.text;
        visuals.widgets.inactive.bg_fill = theme.button;
        visuals.widgets.inactive.weak_bg_fill = theme.button;
        visuals.widgets.inactive.bg_stroke.color = theme.border;
        visuals.widgets.inactive.fg_stroke.color = theme.text;
        visuals.widgets.hovered.bg_fill = theme.button_hover;
        visuals.widgets.hovered.weak_bg_fill = theme.button_hover;
        visuals.widgets.hovered.bg_stroke.color = theme.accent;
        visuals.widgets.hovered.fg_stroke.color = theme.text;
        visuals.widgets.active.bg_fill = theme.button_pressed;
        visuals.widgets.active.weak_bg_fill = theme.button_pressed;
        visuals.widgets.active.bg_stroke.color = theme.accent;
        visuals.widgets.active.fg_stroke.color = theme.text;
        visuals.widgets.open.bg_fill = theme.input;
        visuals.widgets.open.weak_bg_fill = theme.input;
        visuals.widgets.open.bg_stroke.color = theme.accent;
        visuals.widgets.open.fg_stroke.color = theme.text;
        visuals.hyperlink_color = theme.accent_2;
        visuals.selection.bg_fill = theme.accent;
        visuals.selection.stroke.color = contrast_text_color(theme.accent);
        visuals.text_edit_bg_color = Some(theme.input);
        visuals.window_corner_radius = egui::CornerRadius::same(8);
        visuals.menu_corner_radius = egui::CornerRadius::same(8);
        visuals.disabled_alpha = 0.56;

        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(8);
        }

        let mut style = (*ctx.global_style()).clone();
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(12.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(15.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(18.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(14.0, egui::FontFamily::Monospace),
        );
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size = egui::vec2(40.0, 34.0);
        style.spacing.combo_width = 145.0;
        style.spacing.text_edit_width = 280.0;
        style.spacing.scroll = egui::style::ScrollStyle::floating();
        style.spacing.scroll.bar_width = 18.0;
        style.spacing.scroll.floating_width = 5.0;
        style.spacing.scroll.floating_allocated_width = 6.0;
        style.spacing.scroll.handle_min_length = 44.0;
        style.spacing.scroll.active_background_opacity = 0.45;
        style.spacing.scroll.active_handle_opacity = 0.8;
        style.spacing.scroll.interact_background_opacity = 0.85;
        style.spacing.scroll.interact_handle_opacity = 1.0;
        style.visuals = visuals;
        ctx.set_global_style(style);
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

    fn apply_initial_window_fit(&mut self, ctx: &egui::Context) {
        let state = std::mem::replace(&mut self.initial_window_fit, InitialWindowFit::NotNeeded);
        match state {
            InitialWindowFit::NotNeeded => {}
            InitialWindowFit::PendingResize => {
                let Some(monitor_size) = ctx.input(|input| input.viewport().monitor_size) else {
                    self.initial_window_fit = InitialWindowFit::PendingResize;
                    ctx.request_repaint();
                    return;
                };

                if monitor_size.x <= 1.0 || monitor_size.y <= 1.0 {
                    self.initial_window_fit = InitialWindowFit::PendingResize;
                    ctx.request_repaint();
                    return;
                }

                let size = first_launch_window_size(monitor_size);
                let position = egui::pos2(
                    ((monitor_size.x - size.x) / 2.0).max(0.0),
                    ((monitor_size.y - size.y) / 2.0).max(0.0),
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
                self.initial_window_fit = InitialWindowFit::PendingSave { size, frames: 0 };
                ctx.request_repaint();
            }
            InitialWindowFit::PendingSave { size, frames } => {
                let Some(inner_rect) = ctx.input(|input| input.viewport().inner_rect) else {
                    self.initial_window_fit = InitialWindowFit::PendingSave { size, frames };
                    ctx.request_repaint();
                    return;
                };

                let actual_size = inner_rect.size();
                if ((actual_size.x - size.x).abs() <= 2.0 && (actual_size.y - size.y).abs() <= 2.0)
                    || frames >= 5
                {
                    self.initial_window_fit = InitialWindowFit::NotNeeded;
                } else {
                    self.initial_window_fit = InitialWindowFit::PendingSave {
                        size,
                        frames: frames + 1,
                    };
                    ctx.request_repaint();
                }
            }
        }
    }

    fn initial_window_fit_blocks_size_save(&self) -> bool {
        !matches!(self.initial_window_fit, InitialWindowFit::NotNeeded)
    }

    fn remember_window_size(&mut self, ctx: &egui::Context) {
        let Some(inner_rect) = ctx.input(|input| input.viewport().inner_rect) else {
            return;
        };

        let width = inner_rect.width().round().max(1.0);
        let height = inner_rect.height().round().max(1.0);
        let changed = self
            .settings
            .window_width
            .is_none_or(|stored| (stored - width).abs() > 1.0)
            || self
                .settings
                .window_height
                .is_none_or(|stored| (stored - height).abs() > 1.0);

        if changed {
            self.settings.window_width = Some(width);
            self.settings.window_height = Some(height);
            self.save_settings();
        }
    }

    fn follow_link(&mut self, target: &str) {
        let target = target.trim();
        if target.is_empty() {
            return;
        }

        if is_external_link(target) {
            match open::that(target) {
                Ok(()) => self.set_status("Opened external link"),
                Err(error) => self.set_status(format!("Could not open link: {error}")),
            }
            return;
        }

        let Some(current_file) = self.current_file.clone() else {
            return;
        };

        let (path_part, fragment) = split_link_target(target);

        if path_part.is_empty() {
            if let Some(fragment) = fragment {
                self.jump_to_anchor(&fragment);
            }
            return;
        }

        let base_dir = current_file.parent().unwrap_or_else(|| Path::new("."));
        let candidate = if Path::new(&path_part).is_absolute() {
            PathBuf::from(&path_part)
        } else {
            base_dir.join(&path_part)
        };
        let candidate = absolute_path(&candidate);

        if candidate.is_dir() {
            match open::that(&candidate) {
                Ok(()) => self.set_status(format!("Opened folder {}", display_path(&candidate))),
                Err(error) => self.set_status(format!("Could not open folder: {error}")),
            }
            return;
        }

        if candidate.is_file() {
            if is_markdown_file(&candidate) {
                if let Some(fragment) = fragment {
                    self.request_action(PendingAction::OpenFileWithAnchor(candidate, fragment));
                } else {
                    self.request_action(PendingAction::OpenFile(candidate));
                }
            } else if let Err(error) = open::that(&candidate) {
                self.set_status(format!("Could not open file: {error}"));
            }
            return;
        }

        self.set_status(format!("Link target not found: {target}"));
    }

    fn jump_to_anchor(&mut self, fragment: &str) {
        let target_slug = create_heading_slug(fragment);
        for (index, line) in self.render_lines.iter().enumerate() {
            if matches!(line.kind, RenderKind::Heading(_))
                && create_heading_slug(&line.text) == target_slug
            {
                self.pending_scroll_line = Some(index);
                self.set_status(format!("Jumped to #{fragment}"));
                return;
            }
        }

        self.set_status(format!("Anchor not found: #{fragment}"));
    }
}

impl eframe::App for MarkRushApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.draw(ui, frame);
    }
}

fn apply_compact_tree_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.spacing.item_spacing = egui::vec2(4.0, 1.0);
    style.spacing.button_padding = egui::vec2(6.0, 2.0);
    style.spacing.interact_size = egui::vec2(20.0, 20.0);
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(13.0, egui::FontFamily::Proportional),
    );
}

fn show_tree(
    ui: &mut egui::Ui,
    entry: &mut TreeEntry,
    selected_file: Option<&Path>,
    theme: &AppTheme,
) -> Option<TreeAction> {
    if entry.is_dir {
        let should_open = selected_file.is_some_and(|selected| selected.starts_with(&entry.path));
        let response =
            egui::CollapsingHeader::new(egui::RichText::new(&entry.name).color(theme.text))
                .id_salt(entry.path.to_string_lossy().to_string())
                .default_open(should_open)
                .show(ui, |ui| {
                    if let Err(error) = entry.load_children() {
                        ui.label(egui::RichText::new(error.to_string()).color(theme.muted));
                        return None;
                    }

                    for child in &mut entry.children {
                        if let Some(action) = show_tree(ui, child, selected_file, theme) {
                            return Some(action);
                        }
                    }

                    None
                });

        let mut action = response.body_returned.flatten();
        let path = entry.path.clone();
        response.header_response.context_menu(|ui| {
            if ui.button("Show in File Explorer").clicked() {
                action = Some(TreeAction::ShowInBrowser(path.clone()));
                ui.close();
            }
            if ui.button("Copy Folder Path").clicked() {
                action = Some(TreeAction::CopyPath(path.clone()));
                ui.close();
            }
            if ui.button("Rename").clicked() {
                action = Some(TreeAction::Rename(path.clone()));
                ui.close();
            }
        });
        action
    } else {
        let selected = selected_file == Some(entry.path.as_path());
        let response = ui.selectable_label(
            selected,
            egui::RichText::new(&entry.name).color(if selected {
                contrast_text_color(theme.accent)
            } else {
                theme.text
            }),
        );

        let mut action = response
            .clicked()
            .then(|| TreeAction::OpenFile(entry.path.clone()));
        let path = entry.path.clone();
        response.context_menu(|ui| {
            if ui.button("Show in File Explorer").clicked() {
                action = Some(TreeAction::ShowInBrowser(path.clone()));
                ui.close();
            }
            if ui.button("Copy File Path").clicked() {
                action = Some(TreeAction::CopyPath(path.clone()));
                ui.close();
            }
            if ui.button("Rename").clicked() {
                action = Some(TreeAction::Rename(path.clone()));
                ui.close();
            }
        });
        action
    }
}

struct RenderLineOutput {
    clicked_link: Option<String>,
    copied_code: bool,
}

fn draw_render_line(
    ui: &mut egui::Ui,
    line: &RenderLine,
    theme: &AppTheme,
    is_match: bool,
    wrap: bool,
    font_size: f32,
    font_family: &egui::FontFamily,
    copy_feedback_alpha: Option<f32>,
) -> RenderLineOutput {
    let mut clicked_link = None;
    let mut copied_code = false;
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
                    1 => font_size + 7.0,
                    2 => font_size + 4.0,
                    3 => font_size + 2.0,
                    _ => font_size + 0.5,
                };
                ui.label(
                    egui::RichText::new(&line.text)
                        .strong()
                        .font(egui::FontId::new(size, font_family.clone()))
                        .color(text_color),
                );
            }
            RenderKind::Quote => {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("|")
                            .font(egui::FontId::new(font_size, font_family.clone()))
                            .color(theme.accent),
                    );
                    clicked_link = draw_inline_segments(
                        ui,
                        &line.text,
                        theme,
                        theme.muted,
                        wrap,
                        font_size,
                        font_family,
                    );
                });
            }
            RenderKind::CodeBlock { .. } => {
                copied_code = draw_code_block(
                    ui,
                    line,
                    theme,
                    text_color,
                    wrap,
                    font_size,
                    copy_feedback_alpha,
                );
            }
            RenderKind::Rule => {
                ui.separator();
            }
            RenderKind::List | RenderKind::Table | RenderKind::Paragraph => {
                clicked_link = draw_inline_segments(
                    ui,
                    &line.text,
                    theme,
                    text_color,
                    wrap,
                    font_size,
                    font_family,
                );
            }
        }
    });

    RenderLineOutput {
        clicked_link,
        copied_code,
    }
}

fn draw_code_block(
    ui: &mut egui::Ui,
    line: &RenderLine,
    theme: &AppTheme,
    text_color: egui::Color32,
    wrap: bool,
    font_size: f32,
    copy_feedback_alpha: Option<f32>,
) -> bool {
    let (is_first, is_last) = match line.kind {
        RenderKind::CodeBlock { is_first, is_last } => (is_first, is_last),
        _ => (true, true),
    };
    let top_margin = if is_first { 8 } else { 2 };
    let bottom_margin = if is_last { 8 } else { 2 };
    let mut copied_code = false;

    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(CODE_BLOCK_COPY_GUTTER_WIDTH, CODE_BLOCK_COPY_BUTTON_SIZE),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                if is_first {
                    let copy_response = ui
                        .add(
                            egui::Button::new(egui::RichText::new("📋").size(12.0))
                                .fill(theme.button)
                                .min_size(egui::vec2(
                                    CODE_BLOCK_COPY_BUTTON_SIZE,
                                    CODE_BLOCK_COPY_BUTTON_SIZE,
                                )),
                        )
                        .on_hover_text("Copy text block");
                    if copy_response.clicked() {
                        ui.ctx()
                            .copy_text(line.copy_text.clone().unwrap_or_else(|| line.text.clone()));
                        copied_code = true;
                    }
                }
            },
        );

        if let Some(alpha) = copy_feedback_alpha {
            ui.label(
                egui::RichText::new("Copied")
                    .size((font_size - 1.0).max(10.0))
                    .color(theme.accent.gamma_multiply(alpha)),
            );
            ui.add_space(6.0);
        }

        if line.text.is_empty() {
            ui.add_space((font_size * 0.55).max(8.0));
            return;
        }

        egui::Frame::new()
            .fill(theme.code)
            .stroke(egui::Stroke::new(1.0, theme.border))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin {
                left: 10,
                right: 10,
                top: top_margin,
                bottom: bottom_margin,
            })
            .show(ui, |ui| {
                let label = egui::Label::new(
                    egui::RichText::new(line.text.as_str())
                        .font(egui::FontId::new(font_size, egui::FontFamily::Monospace))
                        .color(text_color),
                );
                if wrap {
                    ui.add(label.wrap());
                } else {
                    ui.add(label);
                }
            });
    });

    copied_code
}

fn themed_button(ui: &mut egui::Ui, label: &str, theme: &AppTheme) -> egui::Response {
    ui.add(egui::Button::new(label).fill(theme.button))
}

fn toolbar_icon_button(
    ui: &mut egui::Ui,
    icon: &str,
    tooltip: &str,
    enabled: bool,
    theme: &AppTheme,
) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(icon).size(15.0))
            .fill(theme.button)
            .min_size(egui::vec2(TOPBAR_ICON_BUTTON_SIZE, TOPBAR_ICON_BUTTON_SIZE)),
    )
    .on_hover_text(tooltip)
}

fn draw_inline_segments(
    ui: &mut egui::Ui,
    text: &str,
    theme: &AppTheme,
    text_color: egui::Color32,
    wrap: bool,
    font_size: f32,
    font_family: &egui::FontFamily,
) -> Option<String> {
    let mut clicked_link = None;

    let mut draw_segments = |ui: &mut egui::Ui| {
        let mut remaining = text;
        while let Some((before, label, target, after)) = next_markdown_link(remaining) {
            if !before.is_empty() {
                draw_plain_inline(ui, before, theme, text_color, font_size, font_family);
            }

            let response = ui.link(
                egui::RichText::new(label)
                    .font(egui::FontId::new(font_size, font_family.clone()))
                    .color(theme.accent_2),
            );
            if response.clicked() {
                clicked_link = Some(target.to_owned());
            }

            remaining = after;
        }

        if !remaining.is_empty() {
            draw_plain_inline(ui, remaining, theme, text_color, font_size, font_family);
        }
    };

    if wrap {
        ui.horizontal_wrapped(|ui| draw_segments(ui));
    } else {
        ui.horizontal(|ui| draw_segments(ui));
    }

    clicked_link
}

fn draw_plain_inline(
    ui: &mut egui::Ui,
    text: &str,
    theme: &AppTheme,
    text_color: egui::Color32,
    font_size: f32,
    font_family: &egui::FontFamily,
) {
    let mut remaining = text;

    while !remaining.is_empty() {
        let bold = remaining.find("**").map(|index| (index, "**"));
        let code = remaining.find('`').map(|index| (index, "`"));
        let italic = remaining
            .find('*')
            .filter(|index| {
                !remaining[*index..].starts_with("**")
                    && (*index == 0 || remaining.as_bytes().get(*index - 1) != Some(&b'*'))
            })
            .map(|index| (index, "*"));

        let marker = [bold, code, italic]
            .into_iter()
            .flatten()
            .min_by_key(|(index, _)| *index);

        let Some((index, marker)) = marker else {
            ui.label(
                egui::RichText::new(remaining)
                    .font(egui::FontId::new(font_size, font_family.clone()))
                    .color(text_color),
            );
            break;
        };

        if index > 0 {
            ui.label(
                egui::RichText::new(&remaining[..index])
                    .font(egui::FontId::new(font_size, font_family.clone()))
                    .color(text_color),
            );
        }

        let after_marker = &remaining[index + marker.len()..];
        let Some(end) = after_marker.find(marker) else {
            ui.label(
                egui::RichText::new(&remaining[index..])
                    .font(egui::FontId::new(font_size, font_family.clone()))
                    .color(text_color),
            );
            break;
        };

        let inner = &after_marker[..end];
        match marker {
            "**" => {
                ui.label(
                    egui::RichText::new(inner)
                        .strong()
                        .font(egui::FontId::new(font_size, font_family.clone()))
                        .color(text_color),
                );
            }
            "`" => {
                egui::Frame::new()
                    .fill(theme.code)
                    .inner_margin(egui::Margin::symmetric(4, 1))
                    .show(ui, |ui| {
                        ui.monospace(
                            egui::RichText::new(inner)
                                .font(egui::FontId::new(font_size, font_family.clone()))
                                .color(theme.accent),
                        );
                    });
            }
            "*" => {
                ui.label(
                    egui::RichText::new(inner)
                        .italics()
                        .font(egui::FontId::new(font_size, font_family.clone()))
                        .color(text_color),
                );
            }
            _ => unreachable!(),
        }

        remaining = &after_marker[end + marker.len()..];
    }
}

fn next_markdown_link(text: &str) -> Option<(&str, &str, &str, &str)> {
    let open_bracket = text.find('[')?;
    let after_open = &text[open_bracket + 1..];
    let close_label = after_open.find("](")?;
    let label = &after_open[..close_label];
    let after_label = &after_open[close_label + 2..];
    let close_target = after_label.find(')')?;
    let target = &after_label[..close_target];
    let after = &after_label[close_target + 1..];

    Some((&text[..open_bracket], label, target, after))
}

fn parse_markdown_lines(markdown: &str) -> Vec<RenderLine> {
    let mut lines = Vec::new();
    let mut code_fence = None;
    let mut code_lines = Vec::new();

    for raw_line in markdown.lines() {
        let trimmed = raw_line.trim();

        if let Some(fence) = code_fence {
            if is_closing_code_fence(trimmed, fence) {
                flush_code_block_lines(&mut lines, &mut code_lines);
                code_fence = None;
            } else {
                push_code_block_line(&mut code_lines, raw_line);
            }
            continue;
        }

        if let Some(fence) = parse_code_fence(trimmed) {
            code_fence = Some(fence);
            code_lines.clear();
            continue;
        }

        let (text, kind) = classify_markdown_line(raw_line);
        lines.push(RenderLine::new(text, kind));
    }

    if code_fence.is_some() {
        flush_code_block_lines(&mut lines, &mut code_lines);
    }

    if markdown.ends_with('\n') {
        lines.push(RenderLine::new(String::new(), RenderKind::Blank));
    }

    lines
}

fn push_code_block_line(code_lines: &mut Vec<String>, line: &str) {
    code_lines.push(line.to_owned());
}

fn flush_code_block_lines(lines: &mut Vec<RenderLine>, code_lines: &mut Vec<String>) {
    let copy_text = code_lines.join("\n");
    lines.push(RenderLine::code_block_line(
        copy_text.clone(),
        true,
        true,
        Some(copy_text),
    ));
    code_lines.clear();
}

fn parse_code_fence(line: &str) -> Option<CodeFence> {
    let fence = if line.starts_with("```") {
        CodeFence::Backtick
    } else if line.starts_with("~~~") {
        CodeFence::Tilde
    } else {
        return None;
    };

    let marker = code_fence_marker(fence);
    let marker_len = line.bytes().take_while(|byte| *byte == marker).count();
    if marker_len < 3 {
        return None;
    }

    Some(fence)
}

fn is_closing_code_fence(line: &str, fence: CodeFence) -> bool {
    let marker = code_fence_marker(fence);
    let marker_len = line.bytes().take_while(|byte| *byte == marker).count();

    marker_len >= 3 && line[marker_len..].trim().is_empty()
}

fn code_fence_marker(fence: CodeFence) -> u8 {
    match fence {
        CodeFence::Backtick => b'`',
        CodeFence::Tilde => b'~',
    }
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PrintOutcome {
    Printed,
    Cancelled,
}

#[derive(Clone)]
struct PrintDocument {
    title: String,
    pages: Vec<PrintPage>,
}

#[derive(Clone)]
struct PrintPage {
    lines: Vec<String>,
}

fn build_print_document(title: &str, text: &str) -> PrintDocument {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = Vec::new();

    if normalized.is_empty() {
        lines.push(String::new());
    } else {
        for source_line in normalized.split('\n') {
            lines.extend(wrap_print_preview_line(
                source_line,
                PRINT_PREVIEW_CHARS_PER_LINE,
            ));
        }
    }

    let mut pages: Vec<PrintPage> = lines
        .chunks(PRINT_PREVIEW_LINES_PER_PAGE)
        .map(|chunk| PrintPage {
            lines: chunk.to_vec(),
        })
        .collect();
    if pages.is_empty() {
        pages.push(PrintPage {
            lines: vec![String::new()],
        });
    }

    PrintDocument {
        title: title.to_owned(),
        pages,
    }
}

fn wrap_print_preview_line(line: &str, max_chars: usize) -> Vec<String> {
    let mut pending = line.replace('\t', "    ");
    if pending.is_empty() {
        return vec![String::new()];
    }

    let mut wrapped = Vec::new();
    while pending.chars().count() > max_chars {
        let limit = byte_index_after_chars(&pending, max_chars);
        let candidate = &pending[..limit];
        let split = candidate
            .char_indices()
            .rev()
            .find(|(index, character)| *index > 0 && character.is_whitespace())
            .map(|(index, _)| index)
            .unwrap_or(limit);
        let (head, tail) = pending.split_at(split);
        wrapped.push(head.trim_end().to_owned());
        pending = tail.trim_start().to_owned();

        if pending.is_empty() {
            break;
        }
    }

    if !pending.is_empty() {
        wrapped.push(pending);
    }

    wrapped
}

fn byte_index_after_chars(value: &str, max_chars: usize) -> usize {
    value
        .char_indices()
        .nth(max_chars)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

#[cfg(target_os = "windows")]
fn print_document(document: &PrintDocument) -> Result<PrintOutcome, String> {
    windows_print::print_document(document)
}

#[cfg(not(target_os = "windows"))]
fn print_document(_document: &PrintDocument) -> Result<PrintOutcome, String> {
    Err("Printing is currently available in the Windows build".to_owned())
}

#[cfg(target_os = "windows")]
mod windows_print {
    use super::{PrintDocument, PrintOutcome};
    use std::mem;

    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::Graphics::Gdi::{
        CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DEFAULT_QUALITY, DeleteDC, DeleteObject,
        FF_MODERN, FIXED_PITCH, FW_NORMAL, GetDeviceCaps, GetTextMetricsW, HORZRES, LOGPIXELSX,
        LOGPIXELSY, OUT_DEFAULT_PRECIS, SelectObject, SetBkMode, SetTextColor, TEXTMETRICW,
        TRANSPARENT, TextOutW, VERTRES,
    };
    use windows_sys::Win32::Storage::Xps::{
        AbortDoc, DOCINFOW, EndDoc, EndPage, StartDocW, StartPage,
    };
    use windows_sys::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, PD_HIDEPRINTTOFILE, PD_NOPAGENUMS, PD_NOSELECTION, PD_RETURNDC,
        PD_USEDEVMODECOPIESANDCOLLATE, PRINTDLGW, PrintDlgW,
    };

    struct PrintLayout {
        left: i32,
        top: i32,
        bottom: i32,
        line_height: i32,
    }

    pub fn print_document(document: &PrintDocument) -> Result<PrintOutcome, String> {
        let mut dialog = PRINTDLGW::default();
        dialog.lStructSize = mem::size_of::<PRINTDLGW>() as u32;
        dialog.Flags = PD_RETURNDC
            | PD_NOSELECTION
            | PD_NOPAGENUMS
            | PD_HIDEPRINTTOFILE
            | PD_USEDEVMODECOPIESANDCOLLATE;
        dialog.nCopies = 1;
        dialog.nMinPage = 1;
        dialog.nMaxPage = 1;

        let dialog_result = unsafe { PrintDlgW(&mut dialog) };
        if dialog_result == 0 {
            let error = unsafe { CommDlgExtendedError() };
            free_print_dialog_handles(&dialog);
            if error == 0 {
                return Ok(PrintOutcome::Cancelled);
            }
            return Err(print_dialog_error_message(error));
        }

        if dialog.hDC.is_null() {
            free_print_dialog_handles(&dialog);
            return Err("No printer was selected".to_owned());
        }

        let result = print_to_device(dialog.hDC, document).map(|()| PrintOutcome::Printed);

        unsafe {
            DeleteDC(dialog.hDC);
        }
        free_print_dialog_handles(&dialog);

        result
    }

    fn print_to_device(
        hdc: windows_sys::Win32::Graphics::Gdi::HDC,
        document: &PrintDocument,
    ) -> Result<(), String> {
        let font_name = wide_null("Consolas");
        let dpi_y = device_cap(hdc, LOGPIXELSY, 96);
        let font_height = -((10 * dpi_y + 36) / 72).max(1);
        let font = unsafe {
            CreateFontW(
                font_height,
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                DEFAULT_QUALITY as u32,
                (FIXED_PITCH | FF_MODERN) as u32,
                font_name.as_ptr(),
            )
        };
        if font.is_null() {
            return Err("Could not create printer font".to_owned());
        }

        let previous_font = unsafe { SelectObject(hdc, font) };
        unsafe {
            SetBkMode(hdc, TRANSPARENT as i32);
            SetTextColor(hdc, 0);
        }

        let result = print_with_selected_font(hdc, document);

        unsafe {
            if !previous_font.is_null() {
                SelectObject(hdc, previous_font);
            }
            DeleteObject(font);
        }

        result
    }

    fn print_with_selected_font(
        hdc: windows_sys::Win32::Graphics::Gdi::HDC,
        document: &PrintDocument,
    ) -> Result<(), String> {
        let layout = print_layout(hdc);

        let title_wide = wide_null(&document.title);
        let doc_info = DOCINFOW {
            cbSize: mem::size_of::<DOCINFOW>() as i32,
            lpszDocName: title_wide.as_ptr(),
            lpszOutput: std::ptr::null(),
            lpszDatatype: std::ptr::null(),
            fwType: 0,
        };

        if unsafe { StartDocW(hdc, &doc_info) } <= 0 {
            return Err("Could not start the print job".to_owned());
        }

        let result = print_pages(hdc, &layout, document);
        match result {
            Ok(()) => {
                if unsafe { EndDoc(hdc) } <= 0 {
                    Err("Could not finish the print job".to_owned())
                } else {
                    Ok(())
                }
            }
            Err(error) => {
                unsafe {
                    AbortDoc(hdc);
                }
                Err(error)
            }
        }
    }

    fn print_pages(
        hdc: windows_sys::Win32::Graphics::Gdi::HDC,
        layout: &PrintLayout,
        document: &PrintDocument,
    ) -> Result<(), String> {
        for page in &document.pages {
            if unsafe { StartPage(hdc) } <= 0 {
                return Err("Could not start a printer page".to_owned());
            }

            let mut y = layout.top;
            for line in &page.lines {
                if y + layout.line_height > layout.bottom {
                    break;
                }
                print_line(hdc, layout.left, y, line)?;
                y += layout.line_height;
            }

            if unsafe { EndPage(hdc) } <= 0 {
                return Err("Could not finish a printer page".to_owned());
            }
        }

        Ok(())
    }

    fn print_line(
        hdc: windows_sys::Win32::Graphics::Gdi::HDC,
        x: i32,
        y: i32,
        line: &str,
    ) -> Result<(), String> {
        let wide: Vec<u16> = line.encode_utf16().collect();
        if wide.is_empty() {
            return Ok(());
        }

        let count = wide.len().min(i32::MAX as usize) as i32;
        if unsafe { TextOutW(hdc, x, y, wide.as_ptr(), count) } == 0 {
            Err("Could not draw print text".to_owned())
        } else {
            Ok(())
        }
    }

    fn print_layout(hdc: windows_sys::Win32::Graphics::Gdi::HDC) -> PrintLayout {
        let dpi_x = device_cap(hdc, LOGPIXELSX, 96);
        let dpi_y = device_cap(hdc, LOGPIXELSY, 96);
        let page_width = device_cap(hdc, HORZRES, 8 * dpi_x).max(1);
        let page_height = device_cap(hdc, VERTRES, 10 * dpi_y).max(1);
        let margin_x = (dpi_x / 2).clamp(0, page_width / 3);
        let margin_y = (dpi_y / 2).clamp(0, page_height / 3);
        let line_height = text_line_height(hdc, dpi_y);

        PrintLayout {
            left: margin_x,
            top: margin_y,
            bottom: (page_height - margin_y).max(margin_y + line_height),
            line_height,
        }
    }

    fn text_line_height(hdc: windows_sys::Win32::Graphics::Gdi::HDC, dpi_y: i32) -> i32 {
        let mut metrics = TEXTMETRICW::default();
        if unsafe { GetTextMetricsW(hdc, &mut metrics) } != 0 {
            (metrics.tmHeight + metrics.tmExternalLeading + (dpi_y / 18).max(2)).max(1)
        } else {
            ((11 * dpi_y + 36) / 72 + (dpi_y / 18).max(2)).max(1)
        }
    }

    fn device_cap(hdc: windows_sys::Win32::Graphics::Gdi::HDC, index: u32, fallback: i32) -> i32 {
        let value = unsafe { GetDeviceCaps(hdc, index as i32) };
        if value > 0 { value } else { fallback }
    }

    fn free_print_dialog_handles(dialog: &PRINTDLGW) {
        unsafe {
            if !dialog.hDevMode.is_null() {
                GlobalFree(dialog.hDevMode);
            }
            if !dialog.hDevNames.is_null() {
                GlobalFree(dialog.hDevNames);
            }
        }
    }

    fn print_dialog_error_message(error: u32) -> String {
        match error {
            0x1003 => "No printers are installed".to_owned(),
            0x1004 => "No default printer is available".to_owned(),
            0x1007 => "The selected printer was not found".to_owned(),
            _ => format!("Windows print dialog error 0x{error:04X}"),
        }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

fn show_in_file_browser(path: &Path) -> Result<(), String> {
    let path = absolute_path(path);

    #[cfg(target_os = "windows")]
    {
        let display_path = display_path(&path);
        let mut command = std::process::Command::new("explorer");
        if path.is_file() {
            command.arg(format!("/select,{display_path}"));
        } else {
            command.arg(display_path);
        }

        return command
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let mut command = std::process::Command::new("open");
        if path.is_file() {
            command.arg("-R").arg(&path);
        } else {
            command.arg(&path);
        }

        return command
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let target = if path.is_file() {
            path.parent().unwrap_or(path.as_path())
        } else {
            path.as_path()
        };
        open::that(target).map_err(|error| error.to_string())
    }
}

fn is_external_link(target: &str) -> bool {
    let lower = target.to_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

fn split_link_target(target: &str) -> (String, Option<String>) {
    let mut parts = target.splitn(2, '#');
    let path_part = percent_decode(parts.next().unwrap_or_default().trim());
    let fragment = parts.next().map(|fragment| percent_decode(fragment.trim()));
    (path_part, fragment)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    decoded.push(value);
                    index += 3;
                    continue;
                }
            }
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded).to_string()
}

fn create_heading_slug(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    let mut last_was_hyphen = false;

    for character in text.trim().chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            slug.push(character);
            last_was_hyphen = false;
        } else if character == '_' {
            slug.push(character);
            last_was_hyphen = false;
        } else if (character.is_whitespace() || character == '-')
            && !last_was_hyphen
            && !slug.is_empty()
        {
            slug.push('-');
            last_was_hyphen = true;
        }
    }

    slug.trim_matches('-').to_owned()
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

fn file_modified_time(path: &Path) -> io::Result<SystemTime> {
    fs::metadata(path)?.modified()
}

fn display_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    if let Some(rest) = path.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{rest}")
    } else if let Some(rest) = path.strip_prefix("\\\\?\\") {
        rest.to_owned()
    } else {
        path.into_owned()
    }
}

fn code_copy_feedback_alpha(feedback: Option<CodeCopyFeedback>, line_index: usize) -> Option<f32> {
    let feedback = feedback?;
    if feedback.line_index != line_index {
        return None;
    }

    let elapsed = feedback.started.elapsed();
    if elapsed >= CODE_COPY_FEEDBACK_DURATION {
        return None;
    }

    let progress = elapsed.as_secs_f32() / CODE_COPY_FEEDBACK_DURATION.as_secs_f32();
    Some((1.0 - progress).clamp(0.0, 1.0))
}

fn normalized_prompt_file_name(input: &str, default_markdown_extension: bool) -> Option<String> {
    let mut name = input.trim().to_owned();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    if name.contains('/') || name.contains('\\') {
        return None;
    }
    if name
        .chars()
        .any(|ch| ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
        return None;
    }

    if default_markdown_extension && Path::new(&name).extension().is_none() {
        name.push_str(".md");
    }

    Some(name)
}

fn with_default_markdown_extension(path: PathBuf) -> PathBuf {
    if path.extension().is_some() {
        path
    } else {
        path.with_extension("md")
    }
}

fn unique_markdown_file_name(parent: &Path, base_name: &str) -> String {
    if !parent.join(base_name).exists() {
        return base_name.to_owned();
    }

    let stem = Path::new(base_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("untitled");
    let extension = Path::new(base_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("md");

    for index in 2..1000 {
        let candidate = format!("{stem}-{index}.{extension}");
        if !parent.join(&candidate).exists() {
            return candidate;
        }
    }

    base_name.to_owned()
}

fn middle_scroll_step(distance: f32) -> f32 {
    let magnitude = distance.abs();
    if magnitude <= MIDDLE_SCROLL_DEAD_ZONE {
        return 0.0;
    }

    let speed = ((magnitude - MIDDLE_SCROLL_DEAD_ZONE) / 10.0).powf(1.2) * 4.0;
    distance.signum() * speed.min(MIDDLE_SCROLL_MAX_SPEED)
}

fn sanitize_document_font_size(size: f32) -> f32 {
    if FONT_SIZE_OPTIONS
        .iter()
        .any(|option| (*option - size).abs() < f32::EPSILON)
    {
        size
    } else {
        DEFAULT_DOCUMENT_FONT_SIZE
    }
}

fn document_font_by_label(label: &str) -> &'static DocumentFontChoice {
    document_fonts()
        .iter()
        .find(|font| font.label.eq_ignore_ascii_case(label))
        .unwrap_or(&document_fonts()[0])
}

fn document_fonts() -> &'static [DocumentFontChoice] {
    static FONTS: &[DocumentFontChoice] = &[
        DocumentFontChoice {
            label: "System UI",
            font_key: None,
        },
        DocumentFontChoice {
            label: "Noto Sans",
            font_key: Some("markrush-noto-sans"),
        },
        DocumentFontChoice {
            label: "Noto Serif",
            font_key: Some("markrush-noto-serif"),
        },
        DocumentFontChoice {
            label: "Lora",
            font_key: Some("markrush-lora"),
        },
        DocumentFontChoice {
            label: "Poppins",
            font_key: Some("markrush-poppins"),
        },
        DocumentFontChoice {
            label: "Roboto Mono",
            font_key: Some("markrush-roboto-mono"),
        },
    ];
    FONTS
}

fn reader_row_height(font_size: f32) -> f32 {
    (font_size + 10.0).ceil()
}

fn topbar_left_controls_width(row_width: f32) -> f32 {
    if row_width <= TOPBAR_LEFT_CONTROLS_WIDTH {
        row_width
    } else {
        TOPBAR_LEFT_CONTROLS_WIDTH.min((row_width * 0.35).max(96.0))
    }
}

fn topbar_control_widths(available_width: f32, item_spacing: f32) -> TopbarControlWidths {
    let icon_width = TOPBAR_ICON_BUTTON_SIZE * TOPBAR_FILE_ACTION_COUNT;
    let fixed_spacing = item_spacing * 5.0;
    let combo_budget = (available_width - icon_width - fixed_spacing).max(0.0);
    let minimum_combo_width =
        TOPBAR_MIN_FONT_SIZE_WIDTH + TOPBAR_MIN_FONT_WIDTH + TOPBAR_MIN_THEME_WIDTH;
    let mut extra = (combo_budget - minimum_combo_width).max(0.0);

    let font_size_extra = extra.min(TOPBAR_FONT_SIZE_WIDTH - TOPBAR_MIN_FONT_SIZE_WIDTH);
    extra -= font_size_extra;

    let font_extra = extra.min(TOPBAR_FONT_WIDTH - TOPBAR_MIN_FONT_WIDTH);
    extra -= font_extra;

    let theme_extra = extra.min(TOPBAR_THEME_WIDTH - TOPBAR_MIN_THEME_WIDTH);

    TopbarControlWidths {
        theme: TOPBAR_MIN_THEME_WIDTH + theme_extra,
        font: TOPBAR_MIN_FONT_WIDTH + font_extra,
        font_size: TOPBAR_MIN_FONT_SIZE_WIDTH + font_size_extra,
    }
}

fn reader_estimate_width(available_width: f32) -> f32 {
    (available_width - READER_HORIZONTAL_PADDING).max(180.0)
}

fn estimate_reader_heights(
    lines: &[RenderLine],
    font_size: f32,
    available_width: f32,
    wrap: bool,
) -> Vec<f32> {
    lines
        .iter()
        .map(|line| estimate_reader_line_height(line, font_size, available_width, wrap))
        .collect()
}

fn estimate_reader_line_height(
    line: &RenderLine,
    font_size: f32,
    available_width: f32,
    wrap: bool,
) -> f32 {
    let row_height = reader_row_height(font_size);

    if !wrap {
        return match line.kind {
            RenderKind::Blank => (row_height * 0.65).ceil().max(10.0),
            RenderKind::Heading(level) => {
                let heading_size = match level {
                    1 => font_size + 7.0,
                    2 => font_size + 4.0,
                    3 => font_size + 2.0,
                    _ => font_size + 0.5,
                };
                reader_row_height(heading_size)
            }
            RenderKind::CodeBlock { is_first, .. } => estimate_code_block_line_height(
                &line.text,
                font_size,
                available_width,
                wrap,
                is_first,
            ),
            RenderKind::Rule
            | RenderKind::Quote
            | RenderKind::List
            | RenderKind::Paragraph
            | RenderKind::Table => row_height,
        };
    }

    match line.kind {
        RenderKind::Blank => (row_height * 0.65).ceil().max(10.0),
        RenderKind::Rule => row_height,
        RenderKind::Heading(level) => {
            let heading_size = match level {
                1 => font_size + 7.0,
                2 => font_size + 4.0,
                3 => font_size + 2.0,
                _ => font_size + 0.5,
            };
            estimate_wrapped_text_height(&line.text, heading_size, available_width, 4.0)
                .max(reader_row_height(heading_size))
        }
        RenderKind::Quote => {
            estimate_wrapped_text_height(&line.text, font_size, available_width - 28.0, 6.0)
        }
        RenderKind::CodeBlock { is_first, .. } => {
            estimate_code_block_line_height(&line.text, font_size, available_width, wrap, is_first)
        }
        RenderKind::List | RenderKind::Paragraph | RenderKind::Table => {
            estimate_wrapped_text_height(&line.text, font_size, available_width, 6.0)
                .max(row_height)
        }
    }
}

fn estimate_wrapped_text_height(
    text: &str,
    font_size: f32,
    available_width: f32,
    vertical_padding: f32,
) -> f32 {
    let average_char_width = (font_size * 0.58).max(7.0);
    let chars_per_visual_line = (available_width.max(120.0) / average_char_width)
        .floor()
        .max(8.0);
    let character_count = text.chars().count().max(1) as f32;
    let visual_lines = (character_count / chars_per_visual_line).ceil().max(1.0);

    (visual_lines * (font_size + 7.0) + vertical_padding).ceil()
}

fn estimate_code_block_line_height(
    text: &str,
    font_size: f32,
    available_width: f32,
    wrap: bool,
    is_first: bool,
) -> f32 {
    let average_char_width = (font_size * 0.6).max(7.0);
    let text_width = (available_width - CODE_BLOCK_COPY_GUTTER_WIDTH).max(120.0);
    let chars_per_visual_line = (text_width / average_char_width).floor().max(8.0);
    let visual_lines: f32 = text
        .split('\n')
        .map(|line| {
            if !wrap {
                1.0
            } else {
                let character_count = line.chars().count().max(1) as f32;
                (character_count / chars_per_visual_line).ceil().max(1.0)
            }
        })
        .sum();
    let text_height = visual_lines * (font_size + 7.0) + 14.0;
    if is_first {
        text_height.max(CODE_BLOCK_COPY_BUTTON_SIZE).ceil()
    } else {
        text_height.ceil()
    }
}

fn total_reader_height(heights: &[f32]) -> f32 {
    heights.iter().sum()
}

fn reader_offset_for_line(heights: &[f32], line_index: usize) -> f32 {
    heights.iter().take(line_index).sum()
}

fn line_index_at_reader_offset(heights: &[f32], offset: f32) -> usize {
    let mut y = 0.0;

    for (index, height) in heights.iter().enumerate() {
        if y + *height > offset {
            return index;
        }

        y += *height;
    }

    heights.len().saturating_sub(1)
}

fn selection_scroll_speed(distance: f32, dead_zone: f32, max_speed: f32) -> f32 {
    if distance <= dead_zone {
        return 0.0;
    }

    let span = dead_zone * SELECTION_SCROLL_ZONE_MULTIPLIER;
    let clamped = (distance - dead_zone) / span;
    (clamped.min(1.0).max(0.0) * max_speed).ceil()
}

fn reader_scroll_overscan(viewport_height: f32) -> f32 {
    (viewport_height * READER_OVERSCAN_VIEWPORTS)
        .max(READER_MIN_OVERSCAN)
        .min(READER_MAX_OVERSCAN)
}

fn reader_virtual_range(
    heights: &[f32],
    viewport: egui::Rect,
    overscan: f32,
    total_height: f32,
) -> (usize, usize, f32, f32) {
    let start_target = (viewport.min.y - overscan).max(0.0);
    let end_target = (viewport.max.y + overscan)
        .min(total_height)
        .max(start_target);
    let mut start = 0;
    let mut start_offset = 0.0;

    while start < heights.len() && start_offset + heights[start] < start_target {
        start_offset += heights[start];
        start += 1;
    }

    let mut end = start;
    let mut end_offset = start_offset;
    while end < heights.len() && end_offset < end_target {
        end_offset += heights[end];
        end += 1;
    }

    let bottom_space = (total_height - end_offset).max(0.0);
    (start, end, start_offset, bottom_space)
}

fn document_line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count() + usize::from(text.ends_with('\n'))
    }
}

fn install_system_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    #[cfg(target_os = "windows")]
    {
        if let Ok(bytes) = fs::read("C:\\Windows\\Fonts\\segoeui.ttf") {
            fonts.font_data.insert(
                "segoe-ui".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "segoe-ui".to_owned());
        }

        for path in [
            "C:\\Windows\\Fonts\\CascadiaMono.ttf",
            "C:\\Windows\\Fonts\\CascadiaMonoPL.ttf",
            "C:\\Windows\\Fonts\\consola.ttf",
        ] {
            if let Ok(bytes) = fs::read(path) {
                fonts.font_data.insert(
                    "markrush-monospace".to_owned(),
                    std::sync::Arc::new(egui::FontData::from_owned(bytes)),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .insert(0, "markrush-monospace".to_owned());
                break;
            }
        }
    }

    let proportional_fallbacks = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let monospace_fallbacks = fonts
        .families
        .get(&egui::FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    register_embedded_font(
        &mut fonts,
        "markrush-noto-sans",
        include_bytes!("../assets/fonts/NotoSans.ttf"),
        &proportional_fallbacks,
    );
    register_embedded_font(
        &mut fonts,
        "markrush-noto-serif",
        include_bytes!("../assets/fonts/NotoSerif.ttf"),
        &proportional_fallbacks,
    );
    register_embedded_font(
        &mut fonts,
        "markrush-lora",
        include_bytes!("../assets/fonts/Lora.ttf"),
        &proportional_fallbacks,
    );
    register_embedded_font(
        &mut fonts,
        "markrush-poppins",
        include_bytes!("../assets/fonts/Poppins.ttf"),
        &proportional_fallbacks,
    );
    register_embedded_font(
        &mut fonts,
        "markrush-roboto-mono",
        include_bytes!("../assets/fonts/RobotoMono.ttf"),
        &monospace_fallbacks,
    );

    ctx.set_fonts(fonts);
}

fn register_embedded_font(
    fonts: &mut egui::FontDefinitions,
    font_key: &'static str,
    bytes: &'static [u8],
    fallback_fonts: &[String],
) {
    fonts.font_data.insert(
        font_key.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(bytes)),
    );

    let mut family_fonts = Vec::with_capacity(fallback_fonts.len() + 1);
    family_fonts.push(font_key.to_owned());
    family_fonts.extend(fallback_fonts.iter().cloned());
    fonts
        .families
        .insert(egui::FontFamily::Name(font_key.into()), family_fonts);
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        let log_path = settings_path().and_then(|path| {
            path.parent()
                .map(|parent| parent.join("MarkRush.crash.log"))
        });

        let Some(path) = log_path else {
            return;
        };

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let message = format!("[{:?}]\n{}\n\n", std::time::SystemTime::now(), panic_info);
        let _ = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| {
                use std::io::Write;
                file.write_all(message.as_bytes())
            });
    }));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fenced_code_blocks_hide_fence_markers() {
        let lines = parse_markdown_lines("Before\n```text\nalpha\nbeta\n```\nAfter");

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text, "Before");
        assert!(matches!(
            lines[1].kind,
            RenderKind::CodeBlock {
                is_first: true,
                is_last: true
            }
        ));
        assert_eq!(lines[1].text, "alpha\nbeta");
        assert_eq!(lines[1].copy_text.as_deref(), Some("alpha\nbeta"));
        assert_eq!(lines[2].text, "After");
    }

    #[test]
    fn unclosed_fenced_code_block_still_renders_as_code() {
        let lines = parse_markdown_lines("```json\n{\"ok\":true}");

        assert_eq!(lines.len(), 1);
        assert!(matches!(
            lines[0].kind,
            RenderKind::CodeBlock {
                is_first: true,
                is_last: true
            }
        ));
        assert_eq!(lines[0].text, "{\"ok\":true}");
        assert_eq!(lines[0].copy_text.as_deref(), Some("{\"ok\":true}"));
    }

    #[test]
    fn fenced_code_blocks_preserve_blank_lines() {
        let lines = parse_markdown_lines("```\n\nalpha\n\n```");

        assert_eq!(lines.len(), 1);
        assert!(matches!(
            lines[0].kind,
            RenderKind::CodeBlock {
                is_first: true,
                is_last: true
            }
        ));
        assert_eq!(lines[0].text, "\nalpha\n");
        assert_eq!(lines[0].copy_text.as_deref(), Some("\nalpha\n"));
    }

    #[test]
    fn print_preview_wraps_long_lines() {
        let line = "x".repeat(PRINT_PREVIEW_CHARS_PER_LINE + 8);
        let document = build_print_document("sample.md", &line);

        assert_eq!(document.pages.len(), 1);
        assert_eq!(document.pages[0].lines.len(), 2);
        assert_eq!(
            document.pages[0].lines[0].chars().count(),
            PRINT_PREVIEW_CHARS_PER_LINE
        );
        assert_eq!(document.pages[0].lines[1].chars().count(), 8);
    }

    #[test]
    fn print_preview_splits_pages() {
        let text = (0..(PRINT_PREVIEW_LINES_PER_PAGE + 1))
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let document = build_print_document("sample.md", &text);

        assert_eq!(document.pages.len(), 2);
        assert_eq!(document.pages[0].lines.len(), PRINT_PREVIEW_LINES_PER_PAGE);
        assert_eq!(document.pages[1].lines.len(), 1);
    }
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

fn contrast_text_color(background: egui::Color32) -> egui::Color32 {
    let [r, g, b, _] = background.to_array();
    let luminance = (0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)) / 255.0;

    if luminance > 0.58 {
        egui::Color32::BLACK
    } else {
        egui::Color32::WHITE
    }
}
