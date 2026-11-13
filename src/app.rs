//! Application state, actions and the main frame loop.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, TryRecvError};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Button, CentralPanel, Context, Frame, Key, Layout, Margin, Modifiers, Panel, Pos2,
    ProgressBar, Rect, RichText, Ui, ViewportCommand,
};

use crate::markdown;
use crate::model::{Block, BlockKind, IdGen, Page, PageId, PageMeta};
use crate::search::SearchState;
use crate::store::{Store, export_dir, unix_now};
use crate::theme::{self, Mode, Palette, palette};
use crate::updater::{self, ApplyOutcome, Release, UpdateMsg, UpdateState, UpdateStatus};

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
    BlockContext(crate::editor::BlockContextAction),
    SelectPage(PageId),
    NewPage {
        parent: Option<PageId>,
    },
    NewFolder {
        parent: Option<PageId>,
    },
    MovePage {
        id: PageId,
        parent: Option<PageId>,
        index: usize,
    },
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
    OpenSettings,
    CloseSettings,
    SetTheme(Mode),
    SetZoom(f32),
    SetAutoUpdate(bool),
    SaveNow,
    CopyText(String),
    /// Abre um caminho ou URL com o programa padrão do sistema.
    OpenExternal(String),
    CheckUpdates,
    StartDownload,
    ApplyUpdate,
    DismissUpdate,
    ExportMarkdown(PageId),
    CopyMarkdown(PageId),
    OpenPageMenu(PageId, Pos2),
    OpenEmojiPicker(Pos2),
    SetPageIcon(String),
    ToggleTodo(u64),
    InsertBelow(u64),
    OpenBlockMenu(u64, Pos2),
    Transform(u64, BlockKind),
    OpenCodeLangMenu(u64, Pos2),
    SetBlockLang(u64, String),
    DuplicateBlock(u64),
    DeleteBlock(u64),
    MoveBlock(u64, i32),
    CopyBlock(u64),
    FocusBlock {
        id: u64,
        range: (usize, usize),
    },
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
    pub drag_page: Option<PageId>,
    /// Row the sidebar should scroll into view on the next frame.
    pub sidebar_scroll_to: Option<PageId>,
    /// Movimentos de pagina para Ctrl+Z / Ctrl+Shift+Z na barra lateral.
    pub page_moves: PageMoveHistory,

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
    /// Last laid-out text of each visible block and where it was drawn
    /// (screen space), for ↑/↓ by visual row.
    pub block_galleys: HashMap<u64, (Arc<egui::Galley>, Pos2)>,
    pub title_galley: Option<(Arc<egui::Galley>, Pos2)>,
    /// Caret of the title field: (primary, secondary) char indices.
    pub title_cursor: Option<(usize, usize)>,
    pub hover_block: Option<u64>,
    /// Blocks selected with Shift+↑/↓: (anchor, extent) block ids.
    pub block_selection: Option<(u64, u64)>,
    pub text_span: Option<crate::editor::TextSpan>,
    pub text_anchor: Option<(usize, usize)>,
    pub text_dragging: bool,
    pub native_drag_origin: Option<(usize, usize)>,

    pub slash: Option<SlashState>,
    pub block_menu: Option<(u64, Pos2)>,
    pub block_menu_transform: bool,
    /// Language picker of a code block: (block id, anchor).
    pub code_lang_menu: Option<(u64, Pos2)>,
    pub code_lang_query: String,
    pub code_lang_selected: usize,
    pub code_lang_focus: bool,
    pub page_menu: Option<(PageId, Pos2)>,
    pub emoji_picker: Option<Pos2>,

    pub sidebar_visible: bool,
    /// Ultimo rect da barra lateral (coordenadas de tela), usado para saber
    /// se o ponteiro esta sobre ela nos gestos de pagina.
    pub sidebar_rect: Option<Rect>,
    pub search: SearchState,
    pub shortcuts_open: bool,
    pub markdown_open: bool,
    pub settings_open: bool,
    /// Total de blocos no SQLite, recalculado ao abrir as configurações.
    pub settings_block_count: i64,
    pub confirm_delete: Option<PageId>,

    pub toast: Option<Toast>,
    pub update: UpdateState,
    pub update_rx: Option<mpsc::Receiver<UpdateMsg>>,
    pub popup_guard: f64,
    pub history: History,
    pub scroll_to: Option<u64>,
    pub reset_scroll: bool,
    pub file_hover: bool,
    pub window_title: String,
    pub pending_paste: Option<(u64, String)>,
    pub plain_paste_pending: bool,
    pub plain_paste_until: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub title: String,
    pub icon: String,
    pub blocks: Vec<Block>,
    pub focus: Focus,
    pub cursors: HashMap<u64, (usize, usize)>,
    pub title_cursor: Option<(usize, usize)>,
}

pub struct History {
    pub undo: Vec<Snapshot>,
    pub redo: Vec<Snapshot>,
    pub last: Snapshot,
    pub hash: u64,
    pub pushed_at: f64,
    pub typing: Option<Focus>,
}

const EDIT_HISTORY_LIMIT: usize = 50;
const PAGE_MOVE_HISTORY_LIMIT: usize = 50;

/// Onde uma pagina estava: pai, `sort` gravado e posicao efetiva entre as
/// irmas. O `sort` sozinho nao basta: paginas criadas sem nenhum movimento
/// comecam em 1,2,3..., nao em 0 (ver `Store::create_page`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageSpot {
    pub parent: Option<PageId>,
    pub sort: u32,
    pub index: usize,
}

/// Um movimento de pagina, para desfazer/refazer com `move_page`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageMove {
    pub id: PageId,
    pub from: PageSpot,
    pub to: PageSpot,
}

#[derive(Default)]
pub struct PageMoveHistory {
    pub undo: Vec<PageMove>,
    pub redo: Vec<PageMove>,
}

impl History {
    fn new(page: &Page, now: f64) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            last: snapshot_of(page),
            hash: hash_page(page),
            pushed_at: now,
            typing: None,
        }
    }
}

pub fn snapshot_of(page: &Page) -> Snapshot {
    Snapshot {
        title: page.meta.title.clone(),
        icon: page.meta.icon.clone(),
        blocks: page.blocks.clone(),
        focus: Focus::None,
        cursors: HashMap::new(),
        title_cursor: None,
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

        let mut app = Self::blank();

        // Estado de interface que nao vive no SQLite (issue #6). O tema fica
        // de fora de proposito: ele ja persiste na tabela `settings` e e
        // restaurado em `boot`, e duplicar criaria duas fontes de verdade.
        if let Some(storage) = cc.storage {
            let zoom: Option<f32> = eframe::get_value(storage, "simion_zoom");
            app.zoom = zoom.unwrap_or(1.0).clamp(0.75, 1.6);
            app.sidebar_visible =
                eframe::get_value(storage, "simion_sidebar_visible").unwrap_or(true);
        }

        match Store::open_default() {
            Ok(store) => {
                app.store = Some(store);
                app.boot();
            }
            Err(err) => {
                app.fatal = Some(format!("{err:#}"));
            }
        }
        if app.fatal.is_none() {
            app.init_updater(&cc.egui_ctx);
        }
        app
    }

    /// App state with no store and a placeholder page.
    pub(crate) fn blank() -> Self {
        let mode = Mode::Light;
        Self {
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
            drag_page: None,
            sidebar_scroll_to: None,
            page_moves: PageMoveHistory::default(),
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
            block_galleys: HashMap::new(),
            title_galley: None,
            title_cursor: None,
            hover_block: None,
            block_selection: None,
            text_span: None,
            text_anchor: None,
            text_dragging: false,
            native_drag_origin: None,
            slash: None,
            block_menu: None,
            block_menu_transform: false,
            code_lang_menu: None,
            code_lang_query: String::new(),
            code_lang_selected: 0,
            code_lang_focus: false,
            page_menu: None,
            emoji_picker: None,
            sidebar_visible: true,
            sidebar_rect: None,
            search: SearchState::default(),
            shortcuts_open: false,
            markdown_open: false,
            settings_open: false,
            settings_block_count: 0,
            confirm_delete: None,
            toast: None,
            update: UpdateState::default(),
            update_rx: None,
            popup_guard: 0.0,
            history: History {
                undo: Vec::new(),
                redo: Vec::new(),
                last: Snapshot {
                    title: String::new(),
                    icon: String::new(),
                    blocks: Vec::new(),
                    focus: Focus::None,
                    cursors: HashMap::new(),
                    title_cursor: None,
                },
                hash: 0,
                pushed_at: 0.0,
                typing: None,
            },
            scroll_to: None,
            reset_scroll: true,
            file_hover: false,
            window_title: String::new(),
            pending_paste: None,
            plain_paste_pending: false,
            plain_paste_until: 0.0,
        }
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

        // Expandir/recolher vive em `settings.collapsed_pages`; ids que nao
        // existem mais no banco sao descartados (e limpos da tabela).
        let known: HashSet<PageId> = self.pages.iter().map(|page| page.id).collect();
        let stored =
            parse_collapsed_pages(store.setting("collapsed_pages").ok().flatten().as_deref());
        let filtered: HashSet<PageId> = stored
            .iter()
            .copied()
            .filter(|id| known.contains(id))
            .collect();
        if filtered.len() != stored.len() {
            let value = format_collapsed_pages(&filtered);
            let _ = store.set_setting("collapsed_pages", &value);
        }
        self.collapsed = filtered;

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

    /// Inserts a copy of the block right below it and puts the caret there.
    pub fn duplicate_block(&mut self, id: u64) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        self.begin_change();
        let mut copy = self.page.blocks[index].clone();
        copy.id = self.ids.next();
        let copy_id = copy.id;
        let caret = copy.text.chars().count();
        self.page.blocks.insert(index + 1, copy);
        self.end_change();
        self.focus = Focus::Block(copy_id);
        self.pending = Some(PendingFocus {
            target: Focus::Block(copy_id),
            range: Some((caret, caret)),
            select_all: false,
        });
        self.scroll_to = Some(copy_id);
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
            self.block_galleys.clear();
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
        self.block_galleys.clear();
        self.cursors.clear();
        self.hover_block = None;
        self.block_selection = None;
        self.text_span = None;
        self.text_anchor = None;
        self.text_dragging = false;
        self.native_drag_origin = None;
        self.slash = None;
        self.block_menu = None;
        self.code_lang_menu = None;
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
        self.create_page_with(parent, "", "");
    }

    fn create_folder(&mut self, parent: Option<PageId>) {
        self.create_page_with(parent, "Nova pasta", "📁");
    }

    fn create_page_with(&mut self, parent: Option<PageId>, title: &str, icon: &str) {
        self.save_now();
        let Some(store) = self.store.as_mut() else {
            return;
        };
        match store.create_page(parent, title, icon, unix_now()) {
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
                    self.expand_page(parent);
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
        self.page_moves
            .undo
            .retain(|entry| !removed.contains(&entry.id));
        self.page_moves
            .redo
            .retain(|entry| !removed.contains(&entry.id));
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
        self.prune_collapsed();
    }

    /// Chamadas que mudam a arvore passam por aqui: ajusta `collapsed` ao
    /// banco (ids sumidos) e persiste se algo mudou.
    fn prune_collapsed(&mut self) {
        let known: HashSet<PageId> = self.pages.iter().map(|page| page.id).collect();
        let before = self.collapsed.len();
        self.collapsed.retain(|id| known.contains(id));
        if self.collapsed.len() != before {
            self.persist_collapsed();
        }
    }

    fn persist_collapsed(&mut self) {
        let value = format_collapsed_pages(&self.collapsed);
        if let Some(store) = self.store.as_mut() {
            let _ = store.set_setting("collapsed_pages", &value);
        }
    }

    fn toggle_collapsed(&mut self, id: PageId) {
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
        self.persist_collapsed();
    }

    fn expand_page(&mut self, id: PageId) {
        if self.collapsed.remove(&id) {
            self.persist_collapsed();
        }
    }

    /// Abre os ancestrais para o item movido aparecer na arvore.
    fn reveal_page(&mut self, id: PageId) {
        let ancestors: Vec<PageId> = self
            .path_to(id)
            .into_iter()
            .filter(|ancestor| *ancestor != id)
            .collect();
        let mut changed = false;
        for ancestor in ancestors {
            changed |= self.collapsed.remove(&ancestor);
        }
        if changed {
            self.persist_collapsed();
        }
    }

    fn page_spot(&self, id: PageId) -> Option<PageSpot> {
        let meta = self.meta_of(id)?;
        let mut siblings: Vec<&PageMeta> = self
            .pages
            .iter()
            .filter(|sibling| sibling.parent == meta.parent)
            .collect();
        // Mesma ordem de `Store::sibling_ids` (sort, id); `move_page` usa ela.
        siblings.sort_by_key(|sibling| (sibling.sort, sibling.id));
        let index = siblings.iter().position(|sibling| sibling.id == id)?;
        Some(PageSpot {
            parent: meta.parent,
            sort: meta.sort.max(0) as u32,
            index,
        })
    }

    /// Indice de insercao que devolve `id` ao lugar antigo. As irmas restantes
    /// mantem a ordem relativa, entao o indice gravado antes do movimento
    /// continua valendo (com clamp para sorts burados por exclusoes).
    fn index_for_spot(&self, id: PageId, spot: PageSpot) -> usize {
        let siblings = self
            .pages
            .iter()
            .filter(|meta| meta.parent == spot.parent && meta.id != id)
            .count();
        spot.index.min(siblings)
    }

    fn record_page_move(&mut self, id: PageId, before: PageSpot) {
        let Some(after) = self.page_spot(id) else {
            return;
        };
        if before == after {
            return;
        }
        self.page_moves.undo.push(PageMove {
            id,
            from: before,
            to: after,
        });
        if self.page_moves.undo.len() > PAGE_MOVE_HISTORY_LIMIT {
            self.page_moves.undo.remove(0);
        }
        self.page_moves.redo.clear();
    }

    fn after_page_move(&mut self, id: PageId, before: Option<PageSpot>, remember: bool) {
        self.reload_pages();
        if remember {
            if let Some(before) = before {
                self.record_page_move(id, before);
            }
        }
        if let Some(parent) = self.meta_of(id).and_then(|meta| meta.parent) {
            self.expand_page(parent);
        }
        self.reveal_page(id);
        self.sidebar_scroll_to = Some(id);
    }

    /// Move a pagina e registra o movimento no historico da sidebar.
    pub(crate) fn apply_page_move(
        &mut self,
        id: PageId,
        parent: Option<PageId>,
        index: usize,
    ) -> bool {
        let before = self.page_spot(id);
        let Some(store) = self.store.as_mut() else {
            return false;
        };
        match store.move_page(id, parent, index) {
            Ok(()) => {
                self.after_page_move(id, before, true);
                true
            }
            Err(err) => {
                self.toast(format!("Não foi possível mover: {err}"));
                false
            }
        }
    }

    /// Executa um gesto de navegacao do `Store` (Alt+setas) com o mesmo
    /// bookkeeping de historico/expansao do arrasto.
    pub(crate) fn apply_page_gesture(
        &mut self,
        id: PageId,
        gesture: impl FnOnce(&mut Store) -> anyhow::Result<bool>,
    ) -> bool {
        let before = self.page_spot(id);
        let Some(store) = self.store.as_mut() else {
            return false;
        };
        match gesture(store) {
            Ok(true) => {
                self.after_page_move(id, before, true);
                true
            }
            Ok(false) => false,
            Err(err) => {
                self.toast(format!("Não foi possível mover: {err}"));
                false
            }
        }
    }

    /// Reposiciona sem criar entrada nova (usado por undo/redo).
    fn restore_page_spot(&mut self, id: PageId, spot: PageSpot) -> bool {
        let index = self.index_for_spot(id, spot);
        let Some(store) = self.store.as_mut() else {
            return false;
        };
        match store.move_page(id, spot.parent, index) {
            Ok(()) => {
                self.after_page_move(id, None, false);
                true
            }
            Err(err) => {
                self.toast(format!("Não foi possível mover: {err}"));
                false
            }
        }
    }

    fn undo_page_move(&mut self) {
        let Some(entry) = self.page_moves.undo.pop() else {
            return;
        };
        if !self.restore_page_spot(entry.id, entry.from) {
            return;
        }
        self.page_moves.redo.push(entry);
        if self.page_moves.redo.len() > PAGE_MOVE_HISTORY_LIMIT {
            self.page_moves.redo.remove(0);
        }
        self.toast("Movimento desfeito");
    }

    fn redo_page_move(&mut self) {
        let Some(entry) = self.page_moves.redo.pop() else {
            return;
        };
        if !self.restore_page_spot(entry.id, entry.to) {
            return;
        }
        self.page_moves.undo.push(entry);
        if self.page_moves.undo.len() > PAGE_MOVE_HISTORY_LIMIT {
            self.page_moves.undo.remove(0);
        }
        self.toast("Movimento refeito");
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

    fn set_theme(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.theme_applied = None;
        if let Some(store) = self.store.as_mut() {
            let _ = store.set_setting("theme", self.mode.as_str());
        }
    }

    /// Passo de 0.1 entre 0.75 e 1.6; sempre força reaplicar as fontes.
    pub(crate) fn set_zoom(&mut self, zoom: f32) {
        self.zoom = ((zoom * 100.0).round() / 100.0).clamp(0.75, 1.6);
        self.theme_applied = None;
    }

    fn open_settings(&mut self) {
        self.settings_open = true;
        self.page_menu = None;
        self.refresh_block_count();
    }

    fn refresh_block_count(&mut self) {
        if let Some(store) = self.store.as_ref() {
            self.settings_block_count = store.block_count().unwrap_or(0);
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
    // Updates
    // -----------------------------------------------------------------------

    fn init_updater(&mut self, ctx: &Context) {
        let stored = self
            .store
            .as_ref()
            .and_then(|store| store.setting("auto_update_check").ok().flatten());
        let enabled = parse_auto_update(stored.as_deref());
        let disabled_by_env = std::env::var("SIMION_NO_UPDATE_CHECK")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        self.update.auto_enabled = enabled;
        self.update.env_disabled = disabled_by_env;
        self.update.auto = enabled && !disabled_by_env;
        self.update.last_check = self
            .store
            .as_ref()
            .and_then(|store| store.setting("last_update_check").ok().flatten())
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        self.restore_pending_update();

        if self.update.auto && updater::check_due(self.update.last_check, unix_now()) {
            self.start_update_check(ctx, false);
        }
    }

    /// Reoferece um download concluído numa sessão anterior, sem consultar a
    /// API nem baixar de novo. O arquivo temporário pode ter sido apagado.
    fn restore_pending_update(&mut self) {
        let Some(value) = self
            .store
            .as_ref()
            .and_then(|store| store.setting(updater::PENDING_UPDATE_KEY).ok().flatten())
        else {
            return;
        };
        let pending = updater::parse_pending_update(&value).filter(|(version, path)| {
            path.is_file() && updater::is_newer(version, env!("CARGO_PKG_VERSION"))
        });
        match pending {
            Some((version, path)) => {
                self.update.status = UpdateStatus::Ready {
                    release: updater::pending_release(&version),
                    path,
                };
            }
            None => self.clear_pending_update(),
        }
    }

    fn remember_pending_update(&mut self, release: &Release, path: &Path) {
        if let Some(store) = self.store.as_mut() {
            let value = updater::format_pending_update(&release.version, path);
            let _ = store.set_setting(updater::PENDING_UPDATE_KEY, &value);
        }
    }

    fn clear_pending_update(&mut self) {
        if let Some(store) = self.store.as_mut() {
            // O `Store` só expõe upsert; valor vazio equivale a "sem pendência"
            // para `parse_pending_update`.
            let _ = store.set_setting(updater::PENDING_UPDATE_KEY, "");
        }
    }

    fn start_update_check(&mut self, ctx: &Context, manual: bool) {
        if matches!(&self.update.status, UpdateStatus::Checking) {
            if manual {
                self.toast("Verificando atualizações…");
            }
            return;
        }
        if let UpdateStatus::Downloading { release, .. } = &self.update.status {
            if manual {
                self.toast(format!("Baixando Simion v{}…", release.version));
            }
            return;
        }
        if let UpdateStatus::Ready { release, .. } = &self.update.status {
            if manual {
                self.toast(format!(
                    "Simion v{} já está pronto para instalar",
                    release.version
                ));
            }
            return;
        }

        self.update.auto = !manual;
        self.update.status = UpdateStatus::Checking;
        if manual {
            self.toast("Verificando atualizações…");
        }

        let (sender, receiver) = mpsc::channel();
        self.update_rx = Some(receiver);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = updater::check_latest();
            let _ = sender.send(UpdateMsg::CheckResult(result, manual));
            ctx.request_repaint();
        });
    }

    fn start_download(&mut self, ctx: &Context, release: Release, manual: bool) {
        let asset = match updater::select_asset(&release, updater::detect_platform()) {
            Ok(asset) => asset.clone(),
            Err(err) => {
                if manual {
                    self.update.status = UpdateStatus::Error(err.clone());
                    self.toast(format!("Erro ao atualizar: {err}"));
                } else {
                    self.update.status = UpdateStatus::Idle;
                }
                return;
            }
        };

        self.update.status = UpdateStatus::Downloading {
            release: release.clone(),
            progress: 0.0,
        };
        let (sender, receiver) = mpsc::channel();
        self.update_rx = Some(receiver);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut last = Instant::now();
            let result = updater::download_asset(&asset, |progress| {
                if progress >= 1.0 || last.elapsed() >= Duration::from_millis(80) {
                    last = Instant::now();
                    let _ = sender.send(UpdateMsg::DownloadProgress(release.clone(), progress));
                    ctx.request_repaint();
                }
            });
            let message = match result {
                Ok(path) => UpdateMsg::DownloadReady(release, path),
                Err(err) => UpdateMsg::DownloadError(release, err, manual),
            };
            let _ = sender.send(message);
            ctx.request_repaint();
        });
    }

    fn drain_update_messages(&mut self, ctx: &Context) {
        let mut messages = Vec::new();
        let mut disconnected = false;
        if let Some(receiver) = self.update_rx.as_ref() {
            loop {
                match receiver.try_recv() {
                    Ok(message) => messages.push(message),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        if disconnected {
            self.update_rx = None;
        }
        for message in messages {
            self.handle_update_message(message, ctx);
        }
    }

    fn handle_update_message(&mut self, message: UpdateMsg, ctx: &Context) {
        match message {
            UpdateMsg::CheckResult(Ok(None), manual) => {
                self.update.status = UpdateStatus::UpToDate;
                self.record_update_check();
                if manual {
                    self.toast("Simion está atualizado");
                }
            }
            UpdateMsg::CheckResult(Ok(Some(release)), manual) => {
                self.record_update_check();
                self.update.dismissed = None;
                self.update.status = UpdateStatus::Available(release.clone());
                // A atualização automática baixa sem esperar o usuário.
                self.start_download(ctx, release, manual);
            }
            UpdateMsg::CheckResult(Err(err), manual) => {
                if manual {
                    self.update.status = UpdateStatus::Error(err.clone());
                    self.toast(format!("Erro ao verificar atualizações: {err}"));
                } else {
                    // Uma checagem automática que falha não deve incomodar.
                    self.update.status = UpdateStatus::Idle;
                }
            }
            UpdateMsg::DownloadProgress(release, progress) => {
                if matches!(&self.update.status, UpdateStatus::Downloading { .. }) {
                    self.update.status = UpdateStatus::Downloading { release, progress };
                }
            }
            UpdateMsg::DownloadReady(release, path) => {
                self.remember_pending_update(&release, &path);
                self.update.status = UpdateStatus::Ready { release, path };
                self.toast("Atualização baixada — pronta para instalar");
            }
            UpdateMsg::DownloadError(release, err, manual) => {
                if manual {
                    self.update.status = UpdateStatus::Error(err.clone());
                    self.toast(format!("Erro ao baixar Simion v{}: {err}", release.version));
                } else {
                    self.update.status = UpdateStatus::Idle;
                }
            }
        }
    }

    fn record_update_check(&mut self) {
        let now = unix_now();
        self.update.last_check = now;
        if let Some(store) = self.store.as_mut() {
            let _ = store.set_setting("last_update_check", &now.to_string());
        }
    }

    fn apply_update_now(&mut self) {
        let UpdateStatus::Ready { release, path } = self.update.status.clone() else {
            return;
        };
        // A página precisa estar no SQLite antes de o processo ser reiniciado.
        self.save_now();
        // Portátil: o processo sai dentro de `apply_update`; limpa a pendência
        // antes, mas restaura se a substituição/respawn falhar e o app seguir
        // vivo. Instalador de pacote: mantém a chave para reoferecer ao abrir.
        let portable = updater::replaces_running_app(&path);
        if portable {
            self.clear_pending_update();
        }
        match updater::apply_update(&path) {
            Ok(ApplyOutcome::InstallerOpened) => {
                self.update.status = UpdateStatus::Idle;
                self.update.dismissed = None;
                self.toast("Instalador aberto — conclua a instalação");
            }
            Ok(ApplyOutcome::Restarted) => self.clear_pending_update(),
            Err(err) => {
                // Mantém o estado Ready: o download continua disponível.
                if portable {
                    self.remember_pending_update(&release, &path);
                }
                self.toast(format!(
                    "Não foi possível atualizar para v{}: {err}",
                    release.version
                ));
            }
        }
    }

    fn dismiss_update(&mut self) {
        if let Some(version) = self
            .update
            .status
            .release()
            .map(|release| release.version.clone())
        {
            self.update.dismissed = Some(version);
        } else {
            self.update.status = UpdateStatus::Idle;
        }
    }

    // -----------------------------------------------------------------------
    // History
    // -----------------------------------------------------------------------

    pub fn begin_change(&mut self) {
        self.flush_history(true);
        self.history.typing = None;
        self.history.last = self.editor_snapshot();
    }

    pub fn end_change(&mut self) {
        self.dirty = true;
        self.last_edit = self.now;
        self.flush_history(true);
    }

    pub fn editor_snapshot(&self) -> Snapshot {
        let mut snapshot = snapshot_of(&self.page);
        snapshot.focus = self.focus;
        snapshot.cursors = self.cursors.clone();
        snapshot.title_cursor = self.title_cursor;
        if let Some(pending) = &self.pending {
            snapshot.focus = pending.target;
            if let Some((a, b)) = pending.range {
                match pending.target {
                    Focus::Block(id) => {
                        snapshot.cursors.insert(id, (b, a));
                    }
                    Focus::Title => snapshot.title_cursor = Some((b, a)),
                    Focus::None => {}
                }
            }
        }
        snapshot
    }

    pub fn record_text_change(&mut self, before: Snapshot, typing: bool) {
        let continuation = typing
            && self.history.typing == Some(before.focus)
            && before.cursors == self.history.last.cursors
            && self.now - self.history.pushed_at <= 0.9;
        if !continuation {
            self.history.undo.push(before);
            if self.history.undo.len() > EDIT_HISTORY_LIMIT {
                self.history.undo.remove(0);
            }
        }
        self.history.redo.clear();
        self.history.typing = typing.then_some(self.focus);
        self.history.last = self.editor_snapshot();
        self.history.hash = hash_page(&self.page);
        self.history.pushed_at = self.now;
    }

    pub fn flush_history(&mut self, force: bool) {
        let hash = hash_page(&self.page);
        if hash != self.history.hash {
            self.history.undo.push(self.history.last.clone());
            if self.history.undo.len() > EDIT_HISTORY_LIMIT {
                self.history.undo.remove(0);
            }
            self.history.redo.clear();
            self.history.last = self.editor_snapshot();
            self.history.hash = hash;
            self.history.pushed_at = self.now;
            self.history.typing = None;
        } else if force {
            self.history.last = self.editor_snapshot();
        }
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot) {
        // A stale widget id may refer to a block removed by a structural edit.
        // Preserve its document position before replacing the block list.
        let fallback_index = match snapshot.focus {
            Focus::Block(id) => self.index_of(id),
            _ => None,
        }
        .or_else(|| match self.focus {
            Focus::Block(id) => self.index_of(id),
            _ => None,
        })
        .unwrap_or(0);
        let fallback_cursor = match snapshot.focus {
            Focus::Block(id) => snapshot.cursors.get(&id).copied(),
            _ => None,
        };
        self.text_span = None;
        self.text_anchor = None;
        self.text_dragging = false;
        self.native_drag_origin = None;
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
        self.block_galleys.clear();
        self.cursors = snapshot.cursors;
        self.title_cursor = snapshot.title_cursor;
        self.focus = snapshot.focus;
        if self.focus == Focus::None
            || matches!(self.focus, Focus::Block(id) if self.index_of(id).is_none())
        {
            let block = &self.page.blocks[fallback_index.min(self.page.blocks.len() - 1)];
            self.focus = Focus::Block(block.id);
            self.cursors
                .entry(block.id)
                .or_insert(fallback_cursor.unwrap_or((0, 0)));
        }
        if let Focus::Block(id) = self.focus {
            let chars = self.page.blocks[self.index_of(id).unwrap()]
                .text
                .chars()
                .count();
            let cursor = self.cursors.entry(id).or_insert((0, 0));
            cursor.0 = cursor.0.min(chars);
            cursor.1 = cursor.1.min(chars);
        }
        self.block_selection = None;
        self.slash = None;
        self.history.typing = None;
        let range = match self.focus {
            Focus::Block(id) => self.cursors.get(&id).map(|&(a, b)| (b, a)),
            Focus::Title => self.title_cursor.map(|(a, b)| (b, a)),
            Focus::None => None,
        };
        self.pending = Some(PendingFocus {
            target: self.focus,
            range,
            select_all: false,
        });
        self.scroll_to = match self.focus {
            Focus::Block(id) => Some(id),
            _ => None,
        };
    }

    pub fn undo(&mut self) {
        self.flush_history(true);
        let Some(previous) = self.history.undo.pop() else {
            return;
        };
        self.history.redo.push(self.editor_snapshot());
        self.apply_snapshot(previous);
        self.history.last = self.editor_snapshot();
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
        self.history.undo.push(self.editor_snapshot());
        self.apply_snapshot(next);
        self.history.last = self.editor_snapshot();
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
            Action::BlockContext(action) => self.apply_block_context(action, ctx),
            Action::SelectPage(id) => self.open_page(id),
            Action::NewPage { parent } => self.create_page(parent),
            Action::NewFolder { parent } => self.create_folder(parent),
            Action::MovePage { id, parent, index } => {
                if self.apply_page_move(id, parent, index) {
                    self.toast("Página movida");
                }
            }
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
            Action::ToggleCollapsed(id) => self.toggle_collapsed(id),
            Action::ToggleSidebar => self.sidebar_visible = !self.sidebar_visible,
            Action::ToggleTheme => self.set_theme(self.mode.toggled()),
            Action::SetTheme(mode) => self.set_theme(mode),
            Action::SetZoom(zoom) => self.set_zoom(zoom),
            Action::SetAutoUpdate(on) => {
                if let Some(store) = self.store.as_mut() {
                    let _ = store.set_setting("auto_update_check", auto_update_value(on));
                }
                self.update.auto_enabled = on;
                self.update.auto = on && !self.update.env_disabled;
            }
            Action::OpenSettings => self.open_settings(),
            Action::CloseSettings => self.settings_open = false,
            Action::SaveNow => {
                self.save_now();
                self.toast("Salvo");
                self.refresh_block_count();
            }
            Action::CopyText(text) => {
                ctx.copy_text(text);
                self.toast("Copiado");
            }
            Action::OpenExternal(target) => open_external(&target),
            Action::OpenSearch => {
                self.open_search();
            }
            Action::OpenShortcuts => {
                self.shortcuts_open = true;
                self.page_menu = None;
            }
            Action::OpenMarkdown => {
                self.markdown_open = true;
                self.settings_open = false;
                self.page_menu = None;
            }
            Action::CheckUpdates => self.start_update_check(ctx, true),
            Action::StartDownload => {
                if let UpdateStatus::Available(release) = self.update.status.clone() {
                    let manual = !self.update.auto;
                    self.start_download(ctx, release, manual);
                }
            }
            Action::ApplyUpdate => self.apply_update_now(),
            Action::DismissUpdate => self.dismiss_update(),
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
            Action::OpenCodeLangMenu(id, pos) => {
                self.open_code_lang_menu(id, pos);
                self.block_menu = None;
                self.page_menu = None;
                self.emoji_picker = None;
                self.popup_guard = self.now + 0.2;
            }
            Action::SetBlockLang(id, lang) => self.apply_code_lang(id, &lang),
            Action::DuplicateBlock(id) => {
                self.duplicate_block(id);
                self.block_menu = None;
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
        self.code_lang_menu = None;
        self.emoji_picker = None;
    }

    // -----------------------------------------------------------------------
    // Keyboard
    // -----------------------------------------------------------------------

    fn consume(&self, ctx: &Context, key: Key, modifiers: Modifiers) -> bool {
        self.consume_key(ctx, key, modifiers)
    }

    /// Consumes one press of `key` made with exactly `modifiers`.
    ///
    /// Matches the modifiers carried by the key event itself, not the
    /// modifier state at the end of the frame: when a whole chord (Shift down,
    /// Tab, Shift up) lands in one frame, the frame state is already "no
    /// modifiers" and Shift+Tab would be taken for Tab.
    pub fn consume_key(&self, ctx: &Context, key: Key, modifiers: Modifiers) -> bool {
        ctx.input_mut(|input| {
            let position = input.events.iter().position(|event| {
                matches!(
                    event,
                    egui::Event::Key { key: k, pressed: true, modifiers: m, .. }
                        if *k == key && m.matches_exact(modifiers)
                )
            });
            position.is_some_and(|position| {
                input.events.remove(position);
                true
            })
        })
    }

    /// Gestos da arvore (Alt+setas, Ctrl+Z/Ctrl+Shift+Z de movimento) vivem
    /// na barra lateral. O criterio antigo (`self.focus == Focus::None`) nunca
    /// valia no uso real: abrir uma pagina foca o titulo ou o bloco. Agora o
    /// ponteiro sobre a sidebar basta; fora dela, so quando nenhum campo de
    /// texto tem o teclado, para Alt+setas continuar sendo do editor.
    pub(crate) fn sidebar_hotkeys_allowed(&self, ctx: &Context) -> bool {
        if !self.sidebar_visible {
            return false;
        }
        // O ponteiro manda: com ele sobre a arvore o usuario esta trabalhando
        // na pagina, inclusive logo depois de clicar numa linha (o foco cai
        // no titulo ou no primeiro bloco). Fora dela, so sem foco de texto.
        let pointer_on_sidebar = self.sidebar_rect.is_some_and(|rect| {
            ctx.pointer_hover_pos()
                .is_some_and(|position| rect.contains(position))
        });
        if pointer_on_sidebar {
            return true;
        }
        let text_focused = ctx.memory(|memory| memory.focused()).is_some();
        !text_focused
    }

    pub(crate) fn handle_keys(&mut self, ctx: &Context) {
        self.sync_editor_cursor(ctx);
        // Ctrl+Shift+V: pede o clipboard ao eframe e marca o proximo
        // `Event::Paste` para passar direto ao `TextEdit` focado, sem o
        // parser de Markdown.
        if self.consume(ctx, Key::V, Modifiers::COMMAND | Modifiers::SHIFT) {
            self.plain_paste_pending = true;
            self.plain_paste_until = self.now + 1.0;
            ctx.send_viewport_cmd(ViewportCommand::RequestPaste);
            return;
        }
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
        if self.settings_open {
            if self.consume(ctx, Key::Escape, Modifiers::NONE)
                || self.consume(ctx, Key::Comma, Modifiers::COMMAND)
            {
                self.settings_open = false;
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
        if self.code_lang_menu.is_some() {
            self.code_lang_keys(ctx);
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
        if self.consume(ctx, Key::Comma, Modifiers::COMMAND) {
            self.open_settings();
            return;
        }
        if self.sidebar_hotkeys_allowed(ctx)
            && !self.page_moves.redo.is_empty()
            && self.consume(ctx, Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)
        {
            self.redo_page_move();
            return;
        }
        if self.sidebar_hotkeys_allowed(ctx)
            && !self.page_moves.undo.is_empty()
            && self.consume(ctx, Key::Z, Modifiers::COMMAND)
        {
            self.undo_page_move();
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
            self.set_zoom(self.zoom + 0.1);
            return;
        }
        if self.consume(ctx, Key::Minus, Modifiers::COMMAND) {
            self.set_zoom(self.zoom - 0.1);
            return;
        }
        if self.consume(ctx, Key::Num0, Modifiers::COMMAND) {
            self.set_zoom(1.0);
            return;
        }
        if self.consume(ctx, Key::S, Modifiers::COMMAND) {
            self.apply_action(Action::SaveNow, ctx);
            return;
        }

        if self.sidebar_hotkeys_allowed(ctx) && self.sidebar_keys(ctx) {
            return;
        }
        self.editor_keys(ctx);
    }
}

pub(crate) fn parse_collapsed_pages(value: Option<&str>) -> HashSet<PageId> {
    value
        .unwrap_or_default()
        .split(',')
        .filter_map(|part| part.trim().parse::<PageId>().ok())
        .collect()
}

pub(crate) fn format_collapsed_pages(ids: &HashSet<PageId>) -> String {
    let mut sorted: Vec<PageId> = ids.iter().copied().collect();
    sorted.sort_unstable();
    let parts: Vec<String> = sorted.iter().map(PageId::to_string).collect();
    parts.join(",")
}

/// `settings.auto_update_check`: ausente liga; "0"/"false" desliga.
pub(crate) fn parse_auto_update(value: Option<&str>) -> bool {
    value
        .map(|value| {
            let value = value.trim();
            value != "0" && !value.eq_ignore_ascii_case("false")
        })
        .unwrap_or(true)
}

pub(crate) fn auto_update_value(on: bool) -> &'static str {
    if on { "1" } else { "0" }
}

/// "dd/mm/aaaa hh:mm UTC". Sem dependência de fuso: descobrir o offset local
/// exigiria libc/chrono, então a data fica em UTC e diz isso.
pub(crate) fn format_check_time(ts: i64) -> String {
    if ts <= 0 {
        return "nunca".to_owned();
    }
    // civil_from_days (Howard Hinnant).
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{day:02}/{month:02}/{year:04} {:02}:{:02} UTC",
        secs / 3600,
        secs % 3600 / 60
    )
}

/// Encurta no meio, preservando o começo e o nome do arquivo.
pub(crate) fn truncate_middle(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars || max_chars < 3 {
        return text.to_owned();
    }
    let keep = max_chars - 1;
    let head = keep / 2;
    let tail = keep - head;
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);
    out
}

/// Abre caminho ou URL no programa padrão; falhas são ignoradas.
fn open_external(target: &str) {
    let program = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(target).spawn();
}

/// O clipboard do Windows 11 (Edge/Word/Notepad) entrega `CF_HTML` *e* texto
/// simples; o egui só lê o texto simples. Então decidimos aqui: texto vindo com
/// quebras de linha (documento) ou Markdown de uma linha (título, item) vira
/// blocos. Texto corrido de uma linha — endereço copiado do navegador — fica
/// inline, como no Notion.
pub(crate) fn should_parse_paste(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    text.contains('\n') || crate::markdown::looks_like_markdown(text)
}

impl App {
    pub(crate) fn handle_paste(&mut self, ctx: &Context) {
        if self.plain_paste_pending {
            if self.now > self.plain_paste_until {
                self.plain_paste_pending = false;
            } else {
                let has_paste = ctx.input(|input| {
                    input
                        .events
                        .iter()
                        .any(|event| matches!(event, egui::Event::Paste(_)))
                });
                if has_paste {
                    self.plain_paste_pending = false;
                    // Deixa o TextEdit focado receber o Event::Paste: e o
                    // "colar sem formatar", sem passar pelo parser de Markdown.
                    return;
                }
            }
        }
        let overlays_open = self.search.open
            || self.shortcuts_open
            || self.markdown_open
            || self.settings_open
            || self.block_menu.is_some()
            || self.code_lang_menu.is_some()
            || self.page_menu.is_some()
            || self.emoji_picker.is_some()
            || self.slash.is_some()
            || self.confirm_delete.is_some();
        if overlays_open {
            return;
        }
        if self.block_selection_range().is_some() {
            let mut pasted = None;
            ctx.input_mut(|input| {
                input.events.retain(|event| {
                    if let egui::Event::Paste(text) = event {
                        pasted = Some(text.clone());
                        false
                    } else {
                        true
                    }
                })
            });
            if let Some(text) = pasted {
                self.paste_block_selection(&text);
            }
            return;
        }
        let Focus::Block(block_id) = self.focus else {
            return;
        };
        // Preserve line breaks and the caret; the editor owns block splitting.
        ctx.input_mut(|input| {
            input.events.retain(|event| {
                if let egui::Event::Paste(text) = event
                    && text.contains('\n')
                {
                    self.pending_paste = Some((block_id, text.clone()));
                    false
                } else {
                    true
                }
            });
        });
        if self.pending_paste.is_some() {
            return;
        }
        let mut pasted: Option<String> = None;
        ctx.input(|input| {
            for event in &input.events {
                if let egui::Event::Paste(text) = event {
                    if !input.modifiers.shift && should_parse_paste(text) {
                        pasted = Some(text.clone());
                    }
                }
            }
        });
        ctx.input_mut(|input| {
            let mut kept = Vec::with_capacity(input.events.len());
            for event in std::mem::take(&mut input.events) {
                let keep = match &event {
                    egui::Event::Paste(text) => {
                        !(pasted.as_deref() == Some(text.as_str()) && !input.modifiers.shift)
                    }
                    _ => true,
                };
                if keep {
                    kept.push(event);
                }
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

    /// Faixa fina acima do editor para avisar sobre uma atualização.
    fn draw_update_banner(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let p = self.palette;
        let status = self.update.status.clone();
        let dismissed = self.update.dismissed.clone();
        let visible = match &status {
            UpdateStatus::Available(release)
            | UpdateStatus::Downloading { release, .. }
            | UpdateStatus::Ready { release, .. } => {
                dismissed.as_deref() != Some(release.version.as_str())
            }
            // Erros só aparecem quando a checagem foi pedida pelo usuário.
            UpdateStatus::Error(_) => !self.update.auto,
            _ => false,
        };
        if !visible {
            return;
        }

        Panel::top("simion-update-banner")
            .frame(Frame::new().fill(p.accent_soft).inner_margin(Margin {
                left: 14,
                right: 14,
                top: 8,
                bottom: 8,
            }))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 9.0;
                    match &status {
                        UpdateStatus::Downloading { release, progress } => {
                            ui.label(
                                RichText::new(format!(
                                    "Nova versão v{} disponível",
                                    release.version
                                ))
                                .size(13.0)
                                .color(p.text)
                                .strong(),
                            );
                            ui.add(
                                ProgressBar::new(*progress)
                                    .desired_width(170.0)
                                    .desired_height(10.0)
                                    .fill(p.accent),
                            );
                            ui.label(
                                RichText::new(format!("{:.0}%", *progress * 100.0))
                                    .size(11.5)
                                    .color(p.muted),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if banner_dismiss(ui, &p) {
                                    actions.push(Action::DismissUpdate);
                                }
                            });
                        }
                        UpdateStatus::Ready { release, path } => {
                            ui.label(
                                RichText::new(format!("Atualizar para v{}", release.version))
                                    .size(13.0)
                                    .color(p.text)
                                    .strong(),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let action = if updater::replaces_running_app(path) {
                                    "Reiniciar e atualizar"
                                } else {
                                    "Abrir instalador"
                                };
                                if crate::ui::primary_button(ui, action, &p).clicked() {
                                    actions.push(Action::ApplyUpdate);
                                }
                                if banner_dismiss(ui, &p) {
                                    actions.push(Action::DismissUpdate);
                                }
                            });
                        }
                        UpdateStatus::Available(release) => {
                            ui.label(
                                RichText::new(format!(
                                    "Nova versão v{} disponível",
                                    release.version
                                ))
                                .size(13.0)
                                .color(p.text)
                                .strong(),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if crate::ui::primary_button(ui, "Baixar", &p).clicked() {
                                    actions.push(Action::StartDownload);
                                }
                                if banner_dismiss(ui, &p) {
                                    actions.push(Action::DismissUpdate);
                                }
                            });
                        }
                        UpdateStatus::Error(err) => {
                            ui.label(
                                RichText::new("Falha ao atualizar")
                                    .size(13.0)
                                    .color(p.danger)
                                    .strong(),
                            );
                            ui.label(RichText::new(err.clone()).size(11.5).color(p.muted));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if crate::ui::primary_button(ui, "Tentar de novo", &p).clicked() {
                                    actions.push(Action::CheckUpdates);
                                }
                                if banner_dismiss(ui, &p) {
                                    actions.push(Action::DismissUpdate);
                                }
                            });
                        }
                        _ => {}
                    }
                });
            });
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
        self.drain_update_messages(&ctx);

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
            let sidebar = Panel::left("simion-sidebar")
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
            self.sidebar_rect = Some(sidebar.response.rect);
        } else {
            self.sidebar_rect = None;
        }

        self.draw_update_banner(ui, &mut actions);

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
        // A geometria da janela e gravada pelo proprio eframe (`persist_window`).
        // Aqui vai so o que ele nao conhece. O tema nao entra: ja esta em
        // `settings` no SQLite, restaurado por `boot`.
        eframe::set_value(storage, "simion_zoom", &self.zoom);
        eframe::set_value(storage, "simion_sidebar_visible", &self.sidebar_visible);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette.bg.to_normalized_gamma_f32()
    }
}

fn banner_dismiss(ui: &mut Ui, p: &Palette) -> bool {
    ui.add(Button::new(RichText::new("Depois").size(12.5).color(p.muted)).frame(false))
        .clicked()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsed_setting_round_trips_sorted() {
        let parsed = parse_collapsed_pages(Some(" 12,3, abc,,7 "));
        assert_eq!(parsed, HashSet::from([3, 7, 12]));
        assert_eq!(format_collapsed_pages(&parsed), "3,7,12");
        assert!(parse_collapsed_pages(None).is_empty());
        assert!(parse_collapsed_pages(Some("")).is_empty());
        assert_eq!(format_collapsed_pages(&HashSet::new()), "");
    }

    fn page_order(app: &App, parent: Option<PageId>) -> Vec<PageId> {
        let mut pages: Vec<&PageMeta> = app
            .pages
            .iter()
            .filter(|meta| meta.parent == parent)
            .collect();
        pages.sort_by_key(|meta| (meta.sort, meta.id));
        pages.into_iter().map(|meta| meta.id).collect()
    }

    fn test_meta(id: PageId, parent: Option<PageId>, sort: i64) -> PageMeta {
        PageMeta {
            id,
            parent,
            title: format!("P{id}"),
            icon: String::new(),
            favorite: false,
            sort,
            updated_at: 0,
        }
    }

    /// App com raiz `A B C` e sem as paginas de exemplo do Store.
    fn seeded_root_app(label: &str) -> (App, PathBuf, PageId, PageId, PageId) {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "simion-app-{label}-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut store = Store::open(&path).unwrap();
        for page in store.list_pages().unwrap() {
            store.delete_page(page.id).unwrap();
        }
        let now = unix_now();
        let a = store.create_page(None, "A", "", now).unwrap();
        let b = store.create_page(None, "B", "", now).unwrap();
        let c = store.create_page(None, "C", "", now).unwrap();

        let mut app = App::blank();
        app.store = Some(store);
        app.reload_pages();
        app.current = Some(b.id);
        (app, path, a.id, b.id, c.id)
    }

    /// Roda `handle_keys` com um ponteiro e uma tecla, mantendo um TextEdit
    /// (o titulo aberto) com o teclado, como no uso real.
    fn press_with_focus(
        app: &mut App,
        pointer: Pos2,
        focus: egui::Id,
        key: Key,
        modifiers: Modifiers,
    ) {
        let ctx = Context::default();
        ctx.memory_mut(|memory| memory.request_focus(focus));
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 700.0))),
            events: vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
            ],
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.handle_keys(ui.ctx()));
        output.textures_delta.clear();
    }

    fn title_focus_id(app: &App) -> egui::Id {
        egui::Id::unique(("simion-title", app.current.unwrap()))
    }

    /// Um frame completo do app, com sidebar e editor de verdade. Usado para
    /// conferir o caminho real (rect da sidebar + foco do titulo).
    fn real_ui_frame(ctx: &Context, app: &mut App, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 700.0))),
            events,
            ..Default::default()
        };
        let mut frame = eframe::Frame::_new_kittest();
        let mut output = ctx.run_ui(input, |ui| {
            <App as eframe::App>::ui(app, ui, &mut frame);
        });
        output.textures_delta.clear();
    }

    #[test]
    fn sidebar_alt_down_works_with_open_title_focused() {
        let (mut app, path, a, b, c) = seeded_root_app("sidebar-title-focus");
        let ctx = Context::default();
        crate::theme::install_fonts(&ctx);
        // Abrir a pagina atual pede foco no titulo, como o clique na linha faz.
        app.open_page(b);
        real_ui_frame(&ctx, &mut app, Vec::new());
        real_ui_frame(&ctx, &mut app, Vec::new());
        assert_eq!(app.focus, Focus::Title);
        assert!(ctx.memory(|memory| memory.focused()).is_some());
        let sidebar = app.sidebar_rect.expect("sidebar renderizada");
        assert!(sidebar.contains(Pos2::new(120.0, 120.0)));

        real_ui_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(Pos2::new(120.0, 120.0)),
                egui::Event::Key {
                    key: Key::ArrowDown,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::ALT,
                },
            ],
        );

        assert_eq!(page_order(&app, None), vec![a, c, b]);
        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sidebar_alt_shift_moves_with_open_title_focused() {
        let (mut app, path, a, b, c) = seeded_root_app("sidebar-indent");
        let ctx = Context::default();
        crate::theme::install_fonts(&ctx);
        app.open_page(b);
        real_ui_frame(&ctx, &mut app, Vec::new());
        real_ui_frame(&ctx, &mut app, Vec::new());
        assert_eq!(app.focus, Focus::Title);
        let pointer = Pos2::new(120.0, 120.0);

        real_ui_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::Key {
                    key: Key::ArrowUp,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::ALT | Modifiers::SHIFT,
                },
            ],
        );
        // B vira a ultima filha do irmao anterior (A).
        assert_eq!(page_order(&app, Some(a)), vec![b]);
        assert_eq!(page_order(&app, None), vec![a, c]);

        real_ui_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::Key {
                    key: Key::ArrowDown,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::ALT | Modifiers::SHIFT,
                },
            ],
        );
        // Alt+Shift+↓ desfaz: B volta para a raiz, logo depois de A.
        assert_eq!(page_order(&app, Some(a)), Vec::<PageId>::new());
        assert_eq!(page_order(&app, None), vec![a, b, c]);

        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sidebar_page_move_undo_redo_work_with_title_focused() {
        let (mut app, path, a, b, c) = seeded_root_app("sidebar-undo");
        let ctx = Context::default();
        crate::theme::install_fonts(&ctx);
        app.open_page(b);
        real_ui_frame(&ctx, &mut app, Vec::new());
        real_ui_frame(&ctx, &mut app, Vec::new());
        assert_eq!(app.focus, Focus::Title);
        let pointer = Pos2::new(120.0, 120.0);

        for (key, modifiers) in [
            (Key::ArrowDown, Modifiers::ALT),
            (Key::Z, Modifiers::COMMAND),
            (Key::Z, Modifiers::COMMAND | Modifiers::SHIFT),
        ] {
            real_ui_frame(
                &ctx,
                &mut app,
                vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    },
                ],
            );
            let expected = match (key, modifiers) {
                (Key::ArrowDown, _) => vec![a, c, b],
                (Key::Z, Modifiers::COMMAND) => vec![a, b, c],
                _ => vec![a, c, b],
            };
            assert_eq!(page_order(&app, None), expected);
        }

        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sidebar_gestures_do_not_steal_alt_from_focused_editor() {
        let (mut app, path, a, b, c) = seeded_root_app("sidebar-editor-focus");
        let sidebar = Rect::from_min_size(Pos2::ZERO, egui::vec2(252.0, 700.0));
        app.sidebar_rect = Some(sidebar);
        app.focus = Focus::Title;
        let focus = title_focus_id(&app);

        // Ponteiro no editor e um TextEdit com o teclado: o gesto e do editor.
        press_with_focus(
            &mut app,
            Pos2::new(600.0, 300.0),
            focus,
            Key::ArrowDown,
            Modifiers::ALT,
        );
        assert_eq!(page_order(&app, None), vec![a, b, c]);

        // Ponteiro sobre a sidebar vence o foco do bloco: o usuario clicou na
        // arvore e Alt+setas move a pagina.
        let allowed_over_sidebar =
            sidebar_hotkeys_with_block_focus(&mut app, Pos2::new(120.0, 120.0));
        assert!(allowed_over_sidebar);

        // Fora da sidebar o bloco com o caret mantem Alt+setas para si.
        let allowed_outside = sidebar_hotkeys_with_block_focus(&mut app, Pos2::new(600.0, 300.0));
        assert!(!allowed_outside);

        drop(app);
        let _ = std::fs::remove_file(path);
    }

    /// Foco num bloco (caret no editor) e ponteiro em `pointer`; devolve o gate
    /// dos gestos da arvore.
    fn sidebar_hotkeys_with_block_focus(app: &mut App, pointer: Pos2) -> bool {
        let ctx = Context::default();
        ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique(("simion-block", 7u64))));
        app.focus = Focus::Block(7);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 700.0))),
            events: vec![egui::Event::PointerMoved(pointer)],
            ..Default::default()
        };
        let mut allowed = true;
        let mut output = ctx.run_ui(input, |ui| {
            allowed = app.sidebar_hotkeys_allowed(ui.ctx());
        });
        output.textures_delta.clear();
        allowed
    }

    #[test]
    fn undo_index_uses_saved_position_not_raw_sort() {
        let mut app = App::blank();
        // Depois de mover C para cima: A(0), C(1), B(2).
        app.pages = vec![
            test_meta(1, None, 0),
            test_meta(3, None, 1),
            test_meta(2, None, 2),
        ];
        // C estava na posicao 2 antes do movimento.
        assert_eq!(
            app.index_for_spot(
                3,
                PageSpot {
                    parent: None,
                    sort: 2,
                    index: 2,
                }
            ),
            2
        );
        assert_eq!(
            app.index_for_spot(
                3,
                PageSpot {
                    parent: None,
                    sort: 0,
                    index: 0,
                }
            ),
            0
        );
        // Indice obsoleto (pagina removida) e limitado ao fim da lista.
        assert_eq!(
            app.index_for_spot(
                3,
                PageSpot {
                    parent: None,
                    sort: 99,
                    index: 99,
                }
            ),
            2
        );
    }

    #[test]
    fn undo_restores_moves_when_initial_sorts_are_not_zero_based() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "simion-app-undo-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut store = Store::open(&path).unwrap();
        for page in store.list_pages().unwrap() {
            store.delete_page(page.id).unwrap();
        }
        let now = unix_now();
        let pai = store.create_page(None, "Pai", "", now).unwrap();
        let a = store.create_page(Some(pai.id), "A", "", now).unwrap();
        let b = store.create_page(Some(pai.id), "B", "", now).unwrap();
        let c = store.create_page(Some(pai.id), "C", "", now).unwrap();
        // `create_page` usa MAX(sort)+1: as filhas nascem 1,2,3.
        assert_eq!((a.sort, b.sort, c.sort), (1, 2, 3));

        let mut app = App::blank();
        app.store = Some(store);
        app.reload_pages();
        app.current = Some(b.id);
        assert!(app.apply_page_gesture(b.id, |store| store.move_page_up(b.id)));
        assert_eq!(page_order(&app, Some(pai.id)), vec![b.id, a.id, c.id]);

        app.undo_page_move();
        assert_eq!(page_order(&app, Some(pai.id)), vec![a.id, b.id, c.id]);

        app.redo_page_move();
        assert_eq!(page_order(&app, Some(pai.id)), vec![b.id, a.id, c.id]);

        drop(app);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn auto_update_parsing() {
        assert!(parse_auto_update(None));
        assert!(parse_auto_update(Some("1")));
        assert!(!parse_auto_update(Some("0")));
        assert!(!parse_auto_update(Some(" FALSE ")));
        assert_eq!(auto_update_value(true), "1");
        assert_eq!(auto_update_value(false), "0");
    }

    #[test]
    fn formats_last_check() {
        assert_eq!(format_check_time(0), "nunca");
        assert_eq!(format_check_time(86_399), "01/01/1970 23:59 UTC");
        assert_eq!(format_check_time(951_782_400), "29/02/2000 00:00 UTC");
        assert_eq!(format_check_time(1_700_000_000), "14/11/2023 22:13 UTC");
        assert_eq!(format_check_time(1_798_761_600), "01/01/2027 00:00 UTC");
    }

    #[test]
    fn truncates_in_the_middle() {
        assert_eq!(truncate_middle("abc", 10), "abc");
        assert_eq!(truncate_middle("abcdefghij", 5), "ab…ij");
        assert_eq!(truncate_middle("/home/ção/simion.db", 9).chars().count(), 9);
    }
}
