//! Application state, actions and the main frame loop.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use eframe::egui::{
    self, CentralPanel, Context, Frame, Key, Margin, Modifiers, Panel, Pos2, Rect, RichText, Ui,
    ViewportCommand,
};

use crate::markdown;
use crate::model::{Block, BlockKind, IdGen, Page, PageId, PageMeta};
use crate::search::SearchState;
use crate::store::{Store, export_dir, unix_now};
use crate::theme::{self, Mode, Palette, palette};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    None,
    Title,
    Block(u64),
}

#[derive(Clone, Debug, Default)]
pub struct PendingFocus {
    pub target: Focus,
    /// Character range to select once focused.
    pub range: Option<(usize, usize)>,
    pub select_all: bool,
}

#[derive(Clone, Debug)]
pub struct SlashState {
    pub block: u64,
    /// Character index of the `/`.
    pub start: usize,
    pub query: String,
    pub selected: usize,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub text: String,
    pub until: f64,
}

#[derive(Debug)]
pub enum Action {
    SelectPage(PageId),
    NewPage { parent: Option<PageId> },
    AskDelete(PageId),
    DeletePage(PageId),
    DuplicatePage(PageId),
    FocusTitle,
    ToggleFavorite(PageId),
    ToggleCollapsed(PageId),
    ToggleSidebar,
    ToggleTheme,
    OpenSearch,
    OpenShortcuts,
    OpenMarkdown,
    ExportMarkdown(PageId),
    CopyMarkdown(PageId),
    OpenPageMenu(PageId, Pos2),
    OpenEmojiPicker(Pos2),
    SetPageIcon(String),
    ToggleTodo(u64),
    InsertBelow(u64),
    OpenBlockMenu(u64, Pos2),
    Transform(u64, BlockKind),
    DuplicateBlock(u64),
    DeleteBlock(u64),
    MoveBlock(u64, i32),
    CopyBlock(u64),
    FocusBlock { id: u64, range: (usize, usize) },
}

pub struct App {
    pub store: Option<Store>,
    pub fatal: Option<String>,

    pub now: f64,
    pub mode: Mode,
    pub zoom: f32,
    pub theme_applied: Option<(Mode, u32)>,
    pub palette: Palette,

    pub pages: Vec<PageMeta>,
    pub page: Page,
    pub current: Option<PageId>,
    pub collapsed: HashSet<PageId>,

    pub ids: IdGen,
    pub dirty: bool,
    pub last_edit: f64,
    pub last_save: f64,
    pub saved_flash: f64,

    pub focus: Focus,
    pub pending: Option<PendingFocus>,
    pub cursors: HashMap<u64, (usize, usize)>,
    pub block_rows: HashMap<u64, usize>,
    pub block_heights: HashMap<u64, f32>,
    pub block_rects: HashMap<u64, Rect>,
    pub hover_block: Option<u64>,

    pub slash: Option<SlashState>,
    pub block_menu: Option<(u64, Pos2)>,
    pub block_menu_transform: bool,
    pub page_menu: Option<(PageId, Pos2)>,
    pub emoji_picker: Option<Pos2>,

    pub sidebar_visible: bool,
    pub search: SearchState,
    pub shortcuts_open: bool,
    pub markdown_open: bool,
    pub confirm_delete: Option<PageId>,

    pub toast: Option<Toast>,
    pub popup_guard: f64,
    pub history: History,
    pub scroll_to: Option<u64>,
    pub reset_scroll: bool,
    pub file_hover: bool,
    pub window_title: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub title: String,
    pub icon: String,
    pub blocks: Vec<Block>,
}

pub struct History {
    pub undo: Vec<Snapshot>,
    pub redo: Vec<Snapshot>,
    pub last: Snapshot,
    pub hash: u64,
    pub pushed_at: f64,
}

impl History {
    fn new(page: &Page, now: f64) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            last: snapshot_of(page),
            hash: hash_page(page),
            pushed_at: now,
        }
    }
}

pub fn snapshot_of(page: &Page) -> Snapshot {
    Snapshot {
        title: page.meta.title.clone(),
        icon: page.meta.icon.clone(),
        blocks: page.blocks.clone(),
    }
}

pub fn hash_page(page: &Page) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    page.meta.title.hash(&mut hasher);
    page.meta.icon.hash(&mut hasher);
    for block in &page.blocks {
        block.kind.hash(&mut hasher);
        block.text.hash(&mut hasher);
        block.checked.hash(&mut hasher);
        block.indent.hash(&mut hasher);
        block.lang.hash(&mut hasher);
        block.emoji.hash(&mut hasher);
    }
    hasher.finish()
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_fonts(&cc.egui_ctx);

        let mode = Mode::Light;
        let mut app = Self {
            store: None,
            fatal: None,
            now: 0.0,
            mode,
            zoom: 1.0,
            theme_applied: None,
            palette: palette(mode),
            pages: Vec::new(),
            page: Page::placeholder(),
            current: None,
            collapsed: HashSet::new(),
            ids: IdGen::default(),
            dirty: false,
            last_edit: 0.0,
            last_save: 0.0,
            saved_flash: 0.0,
            focus: Focus::None,
            pending: None,
            cursors: HashMap::new(),
            block_rows: HashMap::new(),
            block_heights: HashMap::new(),
            block_rects: HashMap::new(),
            hover_block: None,
            slash: None,
            block_menu: None,
            block_menu_transform: false,
            page_menu: None,
            emoji_picker: None,
            sidebar_visible: true,
            search: SearchState::default(),
            shortcuts_open: false,
            markdown_open: false,
            confirm_delete: None,
            toast: None,
            popup_guard: 0.0,
            history: History {
                undo: Vec::new(),
                redo: Vec::new(),
                last: Snapshot {
                    title: String::new(),
                    icon: String::new(),
                    blocks: Vec::new(),
                },
                hash: 0,
                pushed_at: 0.0,
            },
            scroll_to: None,
            reset_scroll: true,
            file_hover: false,
            window_title: String::new(),
        };

        match Store::open_default() {
            Ok(store) => {
                app.store = Some(store);
                app.boot();
            }
            Err(err) => {
                app.fatal = Some(format!("{err:#}"));
            }
        }
        app
    }

    fn boot(&mut self) {
        let Some(store) = self.store.as_mut() else {
            return;
        };
        self.mode = store
            .setting("theme")
            .ok()
            .flatten()
            .map(|value| Mode::from_str(&value))
            .unwrap_or(Mode::Light);
        self.palette = palette(self.mode);
        self.pages = store.list_pages().unwrap_or_default();

        if self.pages.is_empty() {
            let _ = store.create_page(None, "", "", unix_now());
            self.pages = store.list_pages().unwrap_or_default();
        }

        let saved = store
            .setting("last_page")
            .ok()
            .flatten()
            .and_then(|value| value.parse::<i64>().ok());
        let target = saved
            .filter(|id| self.pages.iter().any(|page| page.id == *id))
            .or_else(|| {
                self.pages
                    .iter()
                    .find(|page| page.parent.is_none())
                    .map(|page| page.id)
            })
            .or_else(|| self.pages.first().map(|page| page.id));

        if let Some(id) = target {
            self.load_page_from_db(id);
        }
    }

    pub fn title(&self) -> String {
        self.page.meta.display_title().to_owned()
    }

    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.page.blocks.iter().position(|block| block.id == id)
    }

    pub fn meta_of(&self, id: PageId) -> Option<&PageMeta> {
        self.pages.iter().find(|page| page.id == id)
    }

    pub fn children_map(&self) -> HashMap<Option<PageId>, Vec<PageId>> {
        let mut map: HashMap<Option<PageId>, Vec<PageId>> = HashMap::new();
        for page in &self.pages {
            map.entry(page.parent).or_default().push(page.id);
        }
        for list in map.values_mut() {
            list.sort_by(|a, b| {
                let a = self.meta_of(*a).unwrap();
                let b = self.meta_of(*b).unwrap();
                a.sort
                    .cmp(&b.sort)
                    .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            });
        }
        map
    }

    pub fn path_to(&self, id: PageId) -> Vec<PageId> {
        let mut path = Vec::new();
        let mut cursor = Some(id);
        let mut guard = 0;
        while let Some(page_id) = cursor {
            guard += 1;
            if guard > 64 {
                break;
            }
            let Some(meta) = self.meta_of(page_id) else {
                break;
            };
            path.push(meta.id);
            cursor = meta.parent;
        }
        path.reverse();
        path
    }

    fn descendants(&self, id: PageId) -> Vec<PageId> {
        let children = self.children_map();
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            if let Some(list) = children.get(&Some(current)) {
                for child in list {
                    out.push(*child);
                    stack.push(*child);
                }
            }
        }
        out
    }

    pub fn ensure_style(&mut self, ctx: &Context) {
        let key = (self.mode, (self.zoom * 1000.0) as u32);
        if self.theme_applied != Some(key) {
            self.palette = palette(self.mode);
            theme::apply(ctx, &self.palette, self.zoom);
            self.theme_applied = Some(key);
            self.block_heights.clear();
            self.block_rows.clear();
        }
    }

    fn load_page_from_db(&mut self, id: PageId) {
        let loaded = self
            .store
            .as_ref()
            .and_then(|store| store.load_page(id).ok().flatten());
        let Some(mut page) = loaded else {
            self.toast("Não foi possível abrir esta página");
            self.reload_pages();
            return;
        };

        for block in &mut page.blocks {
            block.id = self.ids.next();
        }
        if page.blocks.is_empty() {
            page.blocks
                .push(Block::new(self.ids.next(), BlockKind::Paragraph));
            self.dirty = true;
        }

        self.current = Some(id);
        self.page = page;
        self.history = History::new(&self.page, self.now);
        self.block_heights.clear();
        self.block_rows.clear();
        self.block_rects.clear();
        self.cursors.clear();
        self.hover_block = None;
        self.slash = None;
        self.block_menu = None;
        self.page_menu = None;
        self.emoji_picker = None;
        self.focus = Focus::None;
        self.pending = None;
        self.scroll_to = None;
        self.reset_scroll = true;
        self.dirty = false;
        if let Some(store) = self.store.as_mut() {
            let _ = store.set_setting("last_page", &id.to_string());
        }
        self.window_title.clear();
    }

    pub fn open_page(&mut self, id: PageId) {
        if self.current == Some(id) {
            self.pending = Some(PendingFocus {
                target: Focus::Title,
                range: None,
                select_all: false,
            });
            return;
        }
        self.save_now();
        self.load_page_from_db(id);
    }

    fn create_page(&mut self, parent: Option<PageId>) {
        self.save_now();
        let Some(store) = self.store.as_mut() else {
            return;
        };
        match store.create_page(parent, "", "", unix_now()) {
            Ok(meta) => {
                let page = Page {
                    meta: meta.clone(),
                    blocks: vec![Block::new(self.ids.next(), BlockKind::Paragraph)],
                };
                if let Err(err) = store.save_page(&page) {
                    self.toast(format!("Erro ao criar página: {err}"));
                }
                self.reload_pages();
                self.load_page_from_db(meta.id);
                if let Some(parent) = parent {
                    self.collapsed.remove(&parent);
                }
                self.pending = Some(PendingFocus {
                    target: Focus::Title,
                    range: None,
                    select_all: true,
                });
                self.focus = Focus::Title;
                self.reset_scroll = true;
            }
            Err(err) => self.toast(format!("Erro ao criar página: {err}")),
        }
    }

    fn duplicate_page(&mut self, source: PageId) {
        let Some(meta) = self.meta_of(source).cloned() else {
            return;
        };
        let blocks = if self.current == Some(source) {
            self.page.blocks.clone()
        } else {
            self.store
                .as_ref()
                .and_then(|store| store.load_blocks(source).ok())
                .unwrap_or_default()
        };
        self.save_now();
        let title = format!("{} (cópia)", meta.display_title());
        let created = {
            let Some(store) = self.store.as_mut() else {
                return;
            };
            match store.create_page(meta.parent, &title, &meta.icon, unix_now()) {
                Ok(new_meta) => {
                    let mut blocks = blocks;
                    for block in &mut blocks {
                        block.id = self.ids.next();
                    }
                    if blocks.is_empty() {
                        blocks.push(Block::new(self.ids.next(), BlockKind::Paragraph));
                    }
                    let page = Page {
                        meta: new_meta.clone(),
                        blocks,
                    };
                    if let Err(err) = store.save_page(&page) {
                        self.toast(format!("Erro ao duplicar: {err}"));
                    }
                    Some(new_meta)
                }
                Err(err) => {
                    self.toast(format!("Erro ao duplicar: {err}"));
                    None
                }
            }
        };
        if let Some(meta) = created {
            self.reload_pages();
            self.load_page_from_db(meta.id);
            self.toast("Página duplicada");
        }
    }

    fn delete_page(&mut self, id: PageId) {
        let removed: HashSet<PageId> = std::iter::once(id).chain(self.descendants(id)).collect();
        let affected_current = self
            .current
            .is_some_and(|current| removed.contains(&current));
        let result = self
            .store
            .as_mut()
            .map(|store| store.delete_page(id))
            .unwrap_or(Ok(()));
        if let Err(err) = result {
            self.toast(format!("Erro ao excluir: {err}"));
            return;
        }
        self.reload_pages();
        if affected_current {
            self.current = None;
            self.dirty = false;
            self.history.undo.clear();
            self.history.redo.clear();
            if let Some(next) = self.pages.first().map(|page| page.id) {
                self.load_page_from_db(next);
            } else {
                self.page = Page::placeholder();
                self.scroll_to = None;
                self.focus = Focus::None;
                self.window_title.clear();
            }
        }
        self.toast("Página excluída");
    }

    pub fn reload_pages(&mut self) {
        if let Some(store) = self.store.as_ref() {
            self.pages = store.list_pages().unwrap_or_default();
        }
    }

    pub fn save_now(&mut self) {
        if !self.dirty {
            return;
        }
        let Some(id) = self.current else {
            return;
        };
        self.page.meta.updated_at = unix_now();
        let Some(store) = self.store.as_mut() else {
            return;
        };
        match store.save_page(&self.page) {
            Ok(()) => {
                self.dirty = false;
                self.last_save = self.now;
                self.saved_flash = self.now;
                if let Some(meta) = self.pages.iter_mut().find(|page| page.id == id) {
                    meta.title = self.page.meta.title.clone();
                    meta.icon = self.page.meta.icon.clone();
                    meta.updated_at = self.page.meta.updated_at;
                }
            }
            Err(err) => self.toast(format!("Erro ao salvar: {err}")),
        }
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.toast = Some(Toast {
            text,
            until: self.now + 2.8,
        });
    }

    // -----------------------------------------------------------------------
    // History
    // -----------------------------------------------------------------------

    pub fn begin_change(&mut self) {
        self.flush_history(true);
    }

    pub fn end_change(&mut self) {
        self.dirty = true;
        self.last_edit = self.now;
        self.flush_history(true);
    }

    pub fn flush_history(&mut self, force: bool) {
        let hash = hash_page(&self.page);
        if hash == self.history.hash {
            return;
        }
        if force || self.now - self.history.pushed_at > 0.9 {
            self.history.undo.push(self.history.last.clone());
            if self.history.undo.len() > 200 {
                self.history.undo.remove(0);
            }
            self.history.redo.clear();
            self.history.last = snapshot_of(&self.page);
            self.history.hash = hash;
            self.history.pushed_at = self.now;
        }
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot) {
        self.page.meta.title = snapshot.title;
        self.page.meta.icon = snapshot.icon;
        self.page.blocks = snapshot.blocks;
        for block in &mut self.page.blocks {
            self.ids.observe(block.id);
        }
        if self.page.blocks.is_empty() {
            self.page
                .blocks
                .push(Block::new(self.ids.next(), BlockKind::Paragraph));
        }
        self.block_heights.clear();
        self.block_rows.clear();
        self.block_rects.clear();
        self.cursors.clear();
        if let Focus::Block(id) = self.focus {
            if self.index_of(id).is_none() {
                self.focus = Focus::None;
            }
        }
    }

    pub fn undo(&mut self) {
        self.flush_history(true);
        let Some(previous) = self.history.undo.pop() else {
            return;
        };
        self.history.redo.push(snapshot_of(&self.page));
        self.apply_snapshot(previous);
        self.history.last = snapshot_of(&self.page);
        self.history.hash = hash_page(&self.page);
        self.history.pushed_at = self.now;
        self.dirty = true;
        self.last_edit = self.now;
    }

    pub fn redo(&mut self) {
        self.flush_history(true);
        let Some(next) = self.history.redo.pop() else {
            return;
        };
        self.history.undo.push(snapshot_of(&self.page));
        self.apply_snapshot(next);
        self.history.last = snapshot_of(&self.page);
        self.history.hash = hash_page(&self.page);
        self.history.pushed_at = self.now;
        self.dirty = true;
        self.last_edit = self.now;
    }

    // -----------------------------------------------------------------------
    // Page-level helpers
    // -----------------------------------------------------------------------

    pub fn export_markdown(&mut self, page_id: PageId) {
        self.save_now();
        let (title, markdown_text) = if self.current == Some(page_id) {
            (
                self.title(),
                markdown::page_markdown(&self.title(), &self.page.blocks),
            )
        } else {
            let Some(store) = self.store.as_ref() else {
                return;
            };
            let Some(meta) = store.page_meta(page_id).ok().flatten() else {
                return;
            };
            let blocks = store.load_blocks(page_id).unwrap_or_default();
            let title = meta.display_title().to_owned();
            (title.clone(), markdown::page_markdown(&title, &blocks))
        };
        let dir = export_dir();
        if let Err(err) = std::fs::create_dir_all(&dir) {
            self.toast(format!("Erro ao criar pasta: {err}"));
            return;
        }
        let path = dir.join(format!("{}.md", slugify(&title)));
        match std::fs::write(&path, markdown_text) {
            Ok(()) => self.toast(format!("Exportado para {}", path.display())),
            Err(err) => self.toast(format!("Erro ao exportar: {err}")),
        }
    }

    fn copy_markdown(&mut self, ctx: &Context, page_id: PageId) {
        let markdown_text = if self.current == Some(page_id) {
            markdown::page_markdown(&self.title(), &self.page.blocks)
        } else {
            self.store
                .as_ref()
                .and_then(|store| {
                    let meta = store.page_meta(page_id).ok().flatten()?;
                    let blocks = store.load_blocks(page_id).ok()?;
                    Some(markdown::page_markdown(meta.display_title(), &blocks))
                })
                .unwrap_or_default()
        };
        if markdown_text.trim().is_empty() {
            self.toast("Esta página está vazia");
            return;
        }
        ctx.copy_text(markdown_text);
        self.toast("Markdown copiado");
    }

    fn import_markdown(&mut self, path: PathBuf) {
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                self.toast(format!("Não foi possível ler o arquivo: {err}"));
                return;
            }
        };
        let text = String::from_utf8_lossy(&bytes).to_string();
        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Página importada".to_owned());
        self.save_now();
        let created = {
            let Some(store) = self.store.as_mut() else {
                return;
            };
            match store.create_page(None, &title, "", unix_now()) {
                Ok(meta) => {
                    let mut blocks: Vec<Block> = markdown::parse_blocks(&text, true)
                        .into_iter()
                        .map(|draft| draft.into_block(self.ids.next()))
                        .collect();
                    if blocks.is_empty() {
                        blocks.push(Block::new(self.ids.next(), BlockKind::Paragraph));
                    }
                    let page = Page {
                        meta: meta.clone(),
                        blocks,
                    };
                    if let Err(err) = store.save_page(&page) {
                        self.toast(format!("Erro ao importar: {err}"));
                    }
                    Some(meta)
                }
                Err(err) => {
                    self.toast(format!("Erro ao importar: {err}"));
                    None
                }
            }
        };
        if let Some(meta) = created {
            self.reload_pages();
            self.load_page_from_db(meta.id);
            self.toast(format!("“{title}” importada"));
        }
    }

    // -----------------------------------------------------------------------
    // Actions
    // -----------------------------------------------------------------------

    fn apply_actions(&mut self, actions: Vec<Action>, ctx: &Context) {
        for action in actions {
            self.apply_action(action, ctx);
        }
    }

    fn apply_action(&mut self, action: Action, ctx: &Context) {
        match action {
            Action::SelectPage(id) => self.open_page(id),
            Action::NewPage { parent } => self.create_page(parent),
            Action::AskDelete(id) => {
                self.confirm_delete = Some(id);
                self.page_menu = None;
            }
            Action::DeletePage(id) => {
                self.confirm_delete = None;
                self.delete_page(id);
            }
            Action::DuplicatePage(id) => {
                self.page_menu = None;
                self.duplicate_page(id);
            }
            Action::FocusTitle => {
                self.pending = Some(PendingFocus {
                    target: Focus::Title,
                    range: None,
                    select_all: true,
                });
                self.focus = Focus::Title;
                self.page_menu = None;
            }
            Action::ToggleFavorite(id) => {
                let favorite = self.meta_of(id).map(|meta| meta.favorite).unwrap_or(false);
                if let Some(store) = self.store.as_mut() {
                    let _ = store.set_favorite(id, !favorite);
                }
                if let Some(meta) = self.pages.iter_mut().find(|page| page.id == id) {
                    meta.favorite = !favorite;
                }
                if self.current == Some(id) {
                    self.page.meta.favorite = !favorite;
                }
                self.page_menu = None;
            }
            Action::ToggleCollapsed(id) => {
                if !self.collapsed.remove(&id) {
                    self.collapsed.insert(id);
                }
            }
            Action::ToggleSidebar => self.sidebar_visible = !self.sidebar_visible,
            Action::ToggleTheme => {
                self.mode = self.mode.toggled();
                self.theme_applied = None;
                if let Some(store) = self.store.as_mut() {
                    let _ = store.set_setting("theme", self.mode.as_str());
                }
            }
            Action::OpenSearch => {
                self.open_search();
            }
            Action::OpenShortcuts => {
                self.shortcuts_open = true;
                self.page_menu = None;
            }
            Action::OpenMarkdown => {
                self.markdown_open = true;
                self.page_menu = None;
            }
            Action::ExportMarkdown(id) => {
                self.page_menu = None;
                self.export_markdown(id);
            }
            Action::CopyMarkdown(id) => {
                self.page_menu = None;
                self.copy_markdown(ctx, id);
            }
            Action::OpenPageMenu(id, pos) => {
                self.page_menu = Some((id, pos));
                self.block_menu = None;
                self.emoji_picker = None;
                self.popup_guard = self.now + 0.2;
            }
            Action::OpenEmojiPicker(pos) => {
                self.emoji_picker = Some(pos);
                self.page_menu = None;
                self.popup_guard = self.now + 0.2;
            }
            Action::SetPageIcon(icon) => {
                self.begin_change();
                self.page.meta.icon = icon;
                self.end_change();
                self.save_now();
                self.emoji_picker = None;
            }
            Action::ToggleTodo(id) => {
                if let Some(index) = self.index_of(id) {
                    self.begin_change();
                    self.page.blocks[index].checked = !self.page.blocks[index].checked;
                    self.end_change();
                }
            }
            Action::InsertBelow(id) => {
                if let Some(index) = self.index_of(id) {
                    self.begin_change();
                    let new_id = self.ids.next();
                    let mut block = Block::new(new_id, BlockKind::Paragraph);
                    if self.page.blocks[index].kind.is_list() {
                        block.indent = self.page.blocks[index].indent;
                    }
                    self.page.blocks.insert(index + 1, block);
                    self.end_change();
                    self.pending = Some(PendingFocus {
                        target: Focus::Block(new_id),
                        range: Some((0, 0)),
                        select_all: false,
                    });
                    self.scroll_to = Some(new_id);
                }
            }
            Action::OpenBlockMenu(id, pos) => {
                self.block_menu = Some((id, pos));
                self.block_menu_transform = false;
                self.page_menu = None;
                self.emoji_picker = None;
                self.popup_guard = self.now + 0.2;
            }
            Action::Transform(id, kind) => {
                if let Some(index) = self.index_of(id) {
                    self.begin_change();
                    let block = &mut self.page.blocks[index];
                    block.kind = kind;
                    if kind == BlockKind::Divider {
                        block.text.clear();
                    }
                    if kind == BlockKind::Todo {
                        block.checked = false;
                    }
                    if kind == BlockKind::Callout && block.emoji.is_empty() {
                        block.emoji = "💡".to_owned();
                    }
                    if !kind.is_list() {
                        block.indent = 0;
                    }
                    let caret = block.text.chars().count();
                    self.end_change();
                    self.block_menu = None;
                    self.pending = Some(PendingFocus {
                        target: Focus::Block(id),
                        range: Some((caret, caret)),
                        select_all: false,
                    });
                }
            }
            Action::DuplicateBlock(id) => {
                if let Some(index) = self.index_of(id) {
                    self.begin_change();
                    let mut copy = self.page.blocks[index].clone();
                    copy.id = self.ids.next();
                    self.page.blocks.insert(index + 1, copy.clone());
                    self.end_change();
                    self.block_menu = None;
                    self.pending = Some(PendingFocus {
                        target: Focus::Block(copy.id),
                        range: Some((0, 0)),
                        select_all: false,
                    });
                    self.scroll_to = Some(copy.id);
                }
            }
            Action::DeleteBlock(id) => {
                if let Some(index) = self.index_of(id) {
                    self.begin_change();
                    self.page.blocks.remove(index);
                    if self.page.blocks.is_empty() {
                        self.page
                            .blocks
                            .push(Block::new(self.ids.next(), BlockKind::Paragraph));
                    }
                    let next = self.page.blocks.get(index.min(self.page.blocks.len() - 1));
                    let next_id = next.map(|block| block.id);
                    self.end_change();
                    self.block_menu = None;
                    if let Some(next_id) = next_id {
                        let caret = self
                            .page
                            .blocks
                            .iter()
                            .find(|block| block.id == next_id)
                            .map(|block| block.text.chars().count())
                            .unwrap_or(0);
                        self.pending = Some(PendingFocus {
                            target: Focus::Block(next_id),
                            range: Some((caret, caret)),
                            select_all: false,
                        });
                    }
                }
            }
            Action::MoveBlock(id, delta) => {
                if let Some(index) = self.index_of(id) {
                    let target = index as i64 + delta as i64;
                    if target >= 0 && (target as usize) < self.page.blocks.len() {
                        self.begin_change();
                        let block = self.page.blocks.remove(index);
                        self.page.blocks.insert(target as usize, block);
                        self.end_change();
                        self.block_menu = None;
                        let caret = self.page.blocks[target as usize].text.chars().count();
                        self.pending = Some(PendingFocus {
                            target: Focus::Block(id),
                            range: Some((caret, caret)),
                            select_all: false,
                        });
                    }
                }
            }
            Action::CopyBlock(id) => {
                if let Some(index) = self.index_of(id) {
                    ctx.copy_text(self.page.blocks[index].text.clone());
                    self.toast("Bloco copiado");
                    self.block_menu = None;
                }
            }
            Action::FocusBlock { id, range } => {
                self.pending = Some(PendingFocus {
                    target: Focus::Block(id),
                    range: Some(range),
                    select_all: false,
                });
                self.scroll_to = Some(id);
            }
        }
    }

    pub fn open_search(&mut self) {
        self.save_now();
        self.search.open = true;
        self.search.focus_pending = true;
        self.search.selected = 0;
        self.search.query.clear();
        self.search.results.clear();
        self.search_recent();
        self.page_menu = None;
        self.block_menu = None;
        self.emoji_picker = None;
    }

    // -----------------------------------------------------------------------
    // Keyboard
    // -----------------------------------------------------------------------

    fn consume(&self, ctx: &Context, key: Key, modifiers: Modifiers) -> bool {
        self.consume_key(ctx, key, modifiers)
    }

    pub fn consume_key(&self, ctx: &Context, key: Key, modifiers: Modifiers) -> bool {
        let pressed = ctx.input(|input| input.modifiers);
        if !pressed.matches_exact(modifiers) {
            return false;
        }
        ctx.input_mut(|input| input.consume_key(modifiers, key))
    }

    fn handle_keys(&mut self, ctx: &Context) {
        if self.confirm_delete.is_some() {
            if self.consume(ctx, Key::Escape, Modifiers::NONE) {
                self.confirm_delete = None;
            }
            return;
        }
        if self.shortcuts_open {
            if self.consume(ctx, Key::Escape, Modifiers::NONE)
                || self.consume(ctx, Key::Slash, Modifiers::COMMAND)
            {
                self.shortcuts_open = false;
            }
            return;
        }
        if self.markdown_open {
            if self.consume(ctx, Key::Escape, Modifiers::NONE) {
                self.markdown_open = false;
            }
            return;
        }
        if self.search.open {
            self.search_keys(ctx);
            return;
        }
        if self.slash.is_some() {
            self.slash_keys(ctx);
            return;
        }
        if self.block_menu.is_some() || self.page_menu.is_some() || self.emoji_picker.is_some() {
            if self.consume(ctx, Key::Escape, Modifiers::NONE) {
                self.block_menu = None;
                self.page_menu = None;
                self.emoji_picker = None;
                return;
            }
            let key_pressed = ctx.input(|input| {
                input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key { pressed: true, .. } | egui::Event::Text(_)
                    )
                })
            });
            if key_pressed {
                self.block_menu = None;
                self.page_menu = None;
                self.emoji_picker = None;
            }
            return;
        }

        if self.consume(ctx, Key::K, Modifiers::COMMAND) {
            self.open_search();
            return;
        }
        if self.consume(ctx, Key::N, Modifiers::COMMAND) {
            let parent = self.current;
            self.create_page(parent);
            return;
        }
        if self.consume(ctx, Key::Backslash, Modifiers::COMMAND) {
            self.sidebar_visible = !self.sidebar_visible;
            return;
        }
        if self.consume(ctx, Key::Slash, Modifiers::COMMAND) {
            self.shortcuts_open = true;
            return;
        }
        if self.consume(ctx, Key::Z, Modifiers::COMMAND | Modifiers::SHIFT) {
            self.redo();
            return;
        }
        if self.consume(ctx, Key::Y, Modifiers::COMMAND) {
            self.redo();
            return;
        }
        if self.consume(ctx, Key::Z, Modifiers::COMMAND) {
            self.undo();
            return;
        }
        if self.consume(ctx, Key::M, Modifiers::COMMAND | Modifiers::SHIFT) {
            self.markdown_open = true;
            return;
        }
        if self.consume(ctx, Key::Equals, Modifiers::COMMAND) {
            self.zoom = (self.zoom + 0.1).min(1.6);
            self.theme_applied = None;
            return;
        }
        if self.consume(ctx, Key::Minus, Modifiers::COMMAND) {
            self.zoom = (self.zoom - 0.1).max(0.75);
            self.theme_applied = None;
            return;
        }
        if self.consume(ctx, Key::Num0, Modifiers::COMMAND) {
            self.zoom = 1.0;
            self.theme_applied = None;
            return;
        }
        if self.consume(ctx, Key::S, Modifiers::COMMAND) {
            self.save_now();
            self.toast("Salvo");
            return;
        }

        self.editor_keys(ctx);
    }

    fn handle_paste(&mut self, ctx: &Context) {
        let overlays_open = self.search.open
            || self.shortcuts_open
            || self.markdown_open
            || self.block_menu.is_some()
            || self.page_menu.is_some()
            || self.emoji_picker.is_some()
            || self.slash.is_some()
            || self.confirm_delete.is_some();
        if overlays_open {
            return;
        }
        let Focus::Block(block_id) = self.focus else {
            return;
        };
        let mut pasted: Option<String> = None;
        ctx.input_mut(|input| {
            let mut kept = Vec::with_capacity(input.events.len());
            for event in std::mem::take(&mut input.events) {
                if let egui::Event::Paste(text) = &event {
                    if markdown::looks_like_markdown(text) && !input.modifiers.shift {
                        pasted = Some(text.clone());
                        continue;
                    }
                }
                kept.push(event);
            }
            input.events = kept;
        });
        let Some(text) = pasted else {
            return;
        };
        let Some(index) = self.index_of(block_id) else {
            return;
        };
        let drafts = markdown::parse_blocks(&text, true);
        if drafts.is_empty() {
            return;
        }

        self.begin_change();
        let block_is_empty = self.page.blocks[index].text.trim().is_empty();
        let mut blocks: Vec<Block> = drafts
            .into_iter()
            .map(|draft| draft.into_block(self.ids.next()))
            .collect();
        let insertion_index = if block_is_empty {
            self.page.blocks.remove(index);
            blocks.first().map(|_| index).unwrap_or(index)
        } else {
            index + 1
        };
        let mut at = insertion_index;
        for block in blocks.drain(..) {
            self.page.blocks.insert(at, block);
            at += 1;
        }
        if self.page.blocks.is_empty() {
            self.page
                .blocks
                .push(Block::new(self.ids.next(), BlockKind::Paragraph));
        }
        self.end_change();

        let last_id = self.page.blocks[at.saturating_sub(1).min(self.page.blocks.len() - 1)].id;
        let caret = self
            .page
            .blocks
            .iter()
            .find(|block| block.id == last_id)
            .map(|block| block.text.chars().count())
            .unwrap_or(0);
        self.focus = Focus::Block(last_id);
        self.pending = Some(PendingFocus {
            target: Focus::Block(last_id),
            range: Some((caret, caret)),
            select_all: false,
        });
        self.scroll_to = Some(last_id);
        self.toast("Markdown colado");
    }

    fn handle_files(&mut self, ctx: &Context) {
        self.file_hover = ctx.input(|input| !input.raw.hovered_files.is_empty());
        let dropped: Vec<_> = ctx.input(|input| input.raw.dropped_files.clone());
        for file in dropped {
            let path = file.path().to_path_buf();
            if path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown")
            }) {
                self.import_markdown(path);
            } else {
                self.toast("Solte apenas arquivos .md");
            }
        }
    }

    pub fn after_frame(&mut self, ctx: &Context) {
        self.flush_history(false);

        let idle = self.now - self.last_edit;
        let focused = ctx.input(|input| input.focused);
        if self.dirty && (idle > 0.6 || (!focused && idle > 0.15)) {
            self.save_now();
        }

        let title = if self.current.is_some() {
            format!("{} — Simion", self.title())
        } else {
            "Simion".to_owned()
        };
        if title != self.window_title {
            self.window_title = title.clone();
            ctx.send_viewport_cmd(ViewportCommand::Title(title));
        }

        if ctx.input(|input| input.viewport().close_requested()) {
            self.save_now();
        }

        let toast_active = self
            .toast
            .as_ref()
            .is_some_and(|toast| self.now < toast.until + 0.4);
        if toast_active || self.now - self.saved_flash < 1.6 {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|toast| self.now > toast.until + 0.4)
        {
            self.toast = None;
        }
    }

    // -----------------------------------------------------------------------
    // Frame
    // -----------------------------------------------------------------------

    fn handle_close_request(&mut self) {
        // eframe calls `on_exit`, but saving as soon as the user clicks the
        // close button is safer on every backend.
        self.save_now();
    }

    fn draw_fatal(&mut self, ui: &mut Ui) {
        let p = self.palette;
        CentralPanel::default_margins()
            .frame(Frame::NONE.fill(p.bg))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.label(
                        RichText::new("Não foi possível abrir o banco de dados")
                            .size(22.0)
                            .color(p.text)
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(self.fatal.clone().unwrap_or_default())
                            .color(p.danger)
                            .size(13.0),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("Verifique as permissões da pasta de dados do Simion.")
                            .color(p.muted)
                            .size(13.0),
                    );
                });
            });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.now = ctx.input(|input| input.time);

        if self.fatal.is_some() {
            self.ensure_style(&ctx);
            self.draw_fatal(ui);
            return;
        }

        self.ensure_style(&ctx);
        self.handle_files(&ctx);
        self.handle_paste(&ctx);
        self.handle_keys(&ctx);

        let mut actions: Vec<Action> = Vec::new();
        let p = self.palette;

        if self.sidebar_visible {
            Panel::left("simion-sidebar")
                .resizable(true)
                .default_size(252.0)
                .min_size(196.0)
                .max_size(420.0)
                .show_separator_line(false)
                .frame(Frame::new().fill(p.panel).inner_margin(Margin {
                    left: 10,
                    right: 8,
                    top: 10,
                    bottom: 8,
                }))
                .show(ui, |ui| {
                    self.sidebar(ui, &mut actions);
                });
        }

        Panel::bottom("simion-status")
            .exact_size(28.0)
            .frame(Frame::new().fill(p.bg).inner_margin(Margin {
                left: 0,
                right: 0,
                top: 6,
                bottom: 4,
            }))
            .show(ui, |ui| {
                crate::editor::status_bar(self, ui);
            });

        CentralPanel::default_margins()
            .frame(Frame::NONE.fill(p.bg))
            .show(ui, |ui| {
                self.editor(ui, &mut actions);
            });

        self.apply_actions(actions, &ctx);

        let overlay_actions = self.overlays(&ctx);
        self.apply_actions(overlay_actions, &ctx);

        if ctx.input(|input| input.viewport().close_requested()) {
            self.handle_close_request();
        }

        self.after_frame(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_now();
        if let Some(store) = self.store.as_mut() {
            let _ = store.set_setting("last_page", &self.current.unwrap_or(0).to_string());
        }
    }

    /// Persiste o que o eframe guarda entre execucoes (issue #6).
    ///
    /// `window`/`egui_memory` sao gravados pelo proprio eframe quando a feature
    /// `persistence` esta ativa; aqui acrescentamos o estado do app que tambem
    /// deve sobreviver ao fechamento (zoom, tema e sidebar), para que reabrir o
    /// Simion devolva a mesma janela e o mesmo contexto de trabalho.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "simion_zoom", &self.zoom);
        eframe::set_value(storage, "simion_sidebar_visible", &self.sidebar_visible);
        let mode = self.mode == Mode::Dark;
        eframe::set_value(storage, "simion_dark", &mode);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette.bg.to_normalized_gamma_f32()
    }
}

fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "pagina".to_owned()
    } else {
        trimmed.chars().take(60).collect()
    }
}
