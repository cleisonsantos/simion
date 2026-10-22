//! Core data model: pages and blocks.

pub type PageId = i64;

/// Every block is one of these, much like Notion's block types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum BlockKind {
    #[default]
    Paragraph,
    Heading1,
    Heading2,
    Heading3,
    Bullet,
    Numbered,
    Todo,
    Quote,
    Callout,
    Code,
    Divider,
}

impl BlockKind {
    pub const ALL: [Self; 11] = [
        Self::Paragraph,
        Self::Heading1,
        Self::Heading2,
        Self::Heading3,
        Self::Bullet,
        Self::Numbered,
        Self::Todo,
        Self::Quote,
        Self::Callout,
        Self::Code,
        Self::Divider,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paragraph => "paragraph",
            Self::Heading1 => "h1",
            Self::Heading2 => "h2",
            Self::Heading3 => "h3",
            Self::Bullet => "bullet",
            Self::Numbered => "numbered",
            Self::Todo => "todo",
            Self::Quote => "quote",
            Self::Callout => "callout",
            Self::Code => "code",
            Self::Divider => "divider",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "h1" => Self::Heading1,
            "h2" => Self::Heading2,
            "h3" => Self::Heading3,
            "bullet" => Self::Bullet,
            "numbered" => Self::Numbered,
            "todo" => Self::Todo,
            "quote" => Self::Quote,
            "callout" => Self::Callout,
            "code" => Self::Code,
            "divider" => Self::Divider,
            _ => Self::Paragraph,
        }
    }

    pub fn is_list(self) -> bool {
        matches!(self, Self::Bullet | Self::Numbered | Self::Todo)
    }

    pub fn is_heading(self) -> bool {
        matches!(self, Self::Heading1 | Self::Heading2 | Self::Heading3)
    }

    pub fn accepts_text(self) -> bool {
        self != Self::Divider
    }

    /// Label used in the slash menu & block menu.
    pub fn label(self) -> &'static str {
        match self {
            Self::Paragraph => "Texto",
            Self::Heading1 => "Título 1",
            Self::Heading2 => "Título 2",
            Self::Heading3 => "Título 3",
            Self::Bullet => "Lista com marcadores",
            Self::Numbered => "Lista numerada",
            Self::Todo => "Lista de tarefas",
            Self::Quote => "Citação",
            Self::Callout => "Destaque",
            Self::Code => "Bloco de código",
            Self::Divider => "Divisor",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Paragraph => "Texto simples",
            Self::Heading1 => "Seção grande",
            Self::Heading2 => "Seção média",
            Self::Heading3 => "Seção pequena",
            Self::Bullet => "Lista simples",
            Self::Numbered => "Lista em ordem",
            Self::Todo => "Acompanhe o que fazer",
            Self::Quote => "Destaque uma frase",
            Self::Callout => "Um aviso em destaque",
            Self::Code => "Código com fonte monoespaçada",
            Self::Divider => "Uma linha de separação",
        }
    }

    /// Extra search terms for the slash menu (shortcuts like "h1", "todo").
    pub fn keywords(self) -> &'static str {
        match self {
            Self::Paragraph => "texto paragrafo paragraph p",
            Self::Heading1 => "h1 titulo 1 heading título",
            Self::Heading2 => "h2 titulo 2 heading título",
            Self::Heading3 => "h3 titulo 3 heading título",
            Self::Bullet => "lista marcador bullet ul item",
            Self::Numbered => "lista numerada ol numero",
            Self::Todo => "todo tarefa checkbox check",
            Self::Quote => "citacao quote citar",
            Self::Callout => "destaque callout nota aviso",
            Self::Code => "codigo code bloco trecho",
            Self::Divider => "divisor linha divider separador",
        }
    }

    /// The markdown prefix shown in the slash menu.
    pub fn markdown_hint(self) -> &'static str {
        match self {
            Self::Paragraph => "",
            Self::Heading1 => "#",
            Self::Heading2 => "##",
            Self::Heading3 => "###",
            Self::Bullet => "-",
            Self::Numbered => "1.",
            Self::Todo => "[]",
            Self::Quote => ">",
            Self::Callout => "> [!NOTE]",
            Self::Code => "```",
            Self::Divider => "---",
        }
    }

    /// Font size of the block text, before zoom.
    pub fn font_size(self) -> f32 {
        match self {
            Self::Heading1 => 30.0,
            Self::Heading2 => 24.0,
            Self::Heading3 => 20.0,
            Self::Code => 14.0,
            _ => 16.0,
        }
    }

    pub fn line_height_factor(self) -> f32 {
        match self {
            Self::Heading1 | Self::Heading2 | Self::Heading3 => 1.25,
            Self::Code => 1.6,
            _ => 1.55,
        }
    }

    pub fn vertical_margins(self) -> (f32, f32) {
        match self {
            Self::Heading1 => (16.0, 4.0),
            Self::Heading2 => (12.0, 3.0),
            Self::Heading3 => (9.0, 2.0),
            Self::Code => (0.0, 0.0),
            Self::Callout => (0.0, 0.0),
            _ => (2.0, 2.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Block {
    /// Session-local identity, used for egui widget ids and focus tracking.
    pub id: u64,
    pub kind: BlockKind,
    pub text: String,
    pub checked: bool,
    pub indent: u8,
    pub lang: String,
    pub emoji: String,
}

impl Block {
    pub fn new(id: u64, kind: BlockKind) -> Self {
        Self {
            id,
            kind,
            checked: false,
            indent: 0,
            lang: String::new(),
            emoji: if kind == BlockKind::Callout {
                "💡".to_owned()
            } else {
                String::new()
            },
            text: String::new(),
        }
    }

    #[allow(dead_code)]
    pub fn with_text(id: u64, kind: BlockKind, text: impl Into<String>) -> Self {
        let mut block = Self::new(id, kind);
        block.text = text.into();
        block
    }

    /// Height estimate used before a block has ever been laid out.
    pub fn estimated_height(&self, width: f32, zoom: f32) -> f32 {
        let font = self.kind.font_size() * zoom;
        let line = font * self.kind.line_height_factor();
        let (top, bottom) = self.kind.vertical_margins();
        if self.kind == BlockKind::Divider {
            return 24.0 * zoom;
        }
        let chars_per_line = (width / (font * 0.52)).max(8.0);
        let lines = ((self.text.chars().count() as f32) / chars_per_line)
            .ceil()
            .max(1.0);
        let lines = lines.min(40.0);
        let pad = if matches!(self.kind, BlockKind::Callout | BlockKind::Code) {
            18.0
        } else {
            0.0
        };
        (lines * line + top * zoom + bottom * zoom + pad * zoom).max(24.0 * zoom)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageMeta {
    pub id: PageId,
    pub parent: Option<PageId>,
    pub title: String,
    pub icon: String,
    pub favorite: bool,
    pub sort: i64,
    pub updated_at: i64,
}

impl PageMeta {
    pub fn display_title(&self) -> &str {
        let title = self.title.trim();
        if title.is_empty() {
            "Sem título"
        } else {
            title
        }
    }
}

#[derive(Clone, Debug)]
pub struct Page {
    pub meta: PageMeta,
    pub blocks: Vec<Block>,
}

impl Page {
    pub fn placeholder() -> Self {
        Self {
            meta: PageMeta {
                id: 0,
                parent: None,
                title: String::new(),
                icon: String::new(),
                favorite: false,
                sort: 0,
                updated_at: 0,
            },
            blocks: Vec::new(),
        }
    }
}

/// Monotonic id source for blocks (they are re-created every time a page loads).
#[derive(Debug, Default)]
pub struct IdGen(u64);

impl IdGen {
    pub fn next(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }

    /// Make sure the generator never hands out an id that is already in use.
    pub fn observe(&mut self, id: u64) {
        self.0 = self.0.max(id);
    }
}
