//! SQLite persistence. One local file, no server, no network.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, OptionalExtension, params};

use crate::markdown::parse_blocks;
use crate::model::{Block, BlockKind, Page, PageId, PageMeta};

const SCHEMA: &str = r#"
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS pages (
    id          INTEGER PRIMARY KEY,
    parent_id   INTEGER REFERENCES pages(id) ON DELETE CASCADE,
    title       TEXT NOT NULL DEFAULT '',
    icon        TEXT NOT NULL DEFAULT '',
    favorite    INTEGER NOT NULL DEFAULT 0,
    sort        INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_pages_parent ON pages(parent_id, sort);

CREATE TABLE IF NOT EXISTS blocks (
    page_id     INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    ord         INTEGER NOT NULL,
    kind        TEXT NOT NULL,
    text        TEXT NOT NULL DEFAULT '',
    checked     INTEGER NOT NULL DEFAULT 0,
    indent      INTEGER NOT NULL DEFAULT 0,
    lang        TEXT NOT NULL DEFAULT '',
    emoji       TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (page_id, ord)
);

CREATE TABLE IF NOT EXISTS settings (
    key     TEXT PRIMARY KEY,
    value   TEXT NOT NULL
);
"#;

pub struct Store {
    conn: Connection,
    path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub page_id: PageId,
    pub title: String,
    pub icon: String,
    pub snippet: String,
}

const WELCOME_MD: &str = r#"Um caderno de blocos **local**, rápido e privado. Sem conta, sem nuvem e sem assinatura.

> [!NOTE] Tudo fica em um único arquivo SQLite no seu computador. Seus textos são seus.

## Comece por aqui

- Digite **/** em um bloco vazio para abrir o menu de comandos
- Cole **Markdown** de qualquer lugar: ele virou blocos de verdade
- Selecione um texto e use `Ctrl + B`, `Ctrl + I` ou `Ctrl + E`
- Arraste para selecionar, use `Ctrl + Z` para desfazer

## Uma lista para hoje

- [x] Abrir o Simion
- [ ] Criar a primeira página de verdade
- [ ] Escrever sem medo de perder o texto

## Um exemplo de código

```rust
fn main() {
    println!("Olá, Simion!");
}
```

> "O melhor caderno é aquele que você abre sem pensar."

---

Abra o **Guia de Markdown** na barra lateral para ver o que dá para colar. Para exportar esta página, use o botão de download no topo.
"#;

const GUIDE_MD: &str = r#"# Sintaxe que o Simion entende

O texto dos blocos é **Markdown de verdade**. Você pode digitar os marcadores ou usar os atalhos.

## Ênfase no meio do texto

- **negrito** com `**negrito**`
- *itálico* com `*itálico*`
- ~~riscado~~ com `~~riscado~~`
- `código` com crases
- [um link](https://example.com) com `[texto](url)`

## Atalhos de bloco

Digite no começo de um bloco:

~~~
#   título 1
##  título 2
-   lista
1.  lista numerada
[]  tarefa
>   citação
``` código
--- divisor
~~~

## Colar Markdown

Copie um documento inteiro e cole dentro do Simion: títulos, listas, tarefas, citações e blocos de código são reconhecidos automaticamente.

### Dica final

`Tab` e `Shift + Tab` mudam o nível de recuo. `Alt + ↑/↓` move um bloco.
"#;

impl Store {
    pub fn open_default() -> Result<Self> {
        let path = data_dir().join("simion.db");
        Self::open(&path)
    }

    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("criando {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("abrindo o banco em {}", path.display()))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 3000;",
        )?;
        conn.execute_batch(SCHEMA)?;

        conn.create_scalar_function(
            "fold",
            1,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            |ctx| {
                let value: String = ctx.get(0)?;
                Ok(fold(&value))
            },
        )?;
        let mut store = Self {
            conn,
            path: path.to_path_buf(),
        };
        store.seed_if_empty()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_empty(&self) -> Result<bool> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM pages", [], |row| row.get(0))?;
        Ok(count == 0)
    }

    /// Creates the two starter pages on first launch.
    fn seed_if_empty(&mut self) -> Result<()> {
        if !self.is_empty()? {
            return Ok(());
        }
        let now = unix_now();
        let welcome = self.create_page(None, "Primeiros passos", "🌱", now)?;
        let blocks: Vec<Block> = parse_blocks(WELCOME_MD, true)
            .into_iter()
            .enumerate()
            .map(|(i, draft)| draft.into_block(i as u64 + 1))
            .collect();
        let page = Page {
            meta: welcome.clone(),
            blocks,
        };
        self.save_page(&page)?;

        let guide = self.create_page(Some(welcome.id), "Guia de Markdown", "📝", now)?;
        let blocks: Vec<Block> = parse_blocks(GUIDE_MD, true)
            .into_iter()
            .enumerate()
            .map(|(i, draft)| draft.into_block(i as u64 + 1))
            .collect();
        let page = Page {
            meta: guide,
            blocks,
        };
        self.save_page(&page)?;

        self.set_setting("last_page", &welcome.id.to_string())?;
        self.set_setting("theme", "light")?;
        Ok(())
    }

    pub fn list_pages(&self) -> Result<Vec<PageMeta>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, parent_id, title, icon, favorite, sort, updated_at
             FROM pages ORDER BY sort ASC, title COLLATE NOCASE ASC, id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PageMeta {
                id: row.get(0)?,
                parent: row.get(1)?,
                title: row.get(2)?,
                icon: row.get(3)?,
                favorite: row.get::<_, i64>(4)? != 0,
                sort: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn page_meta(&self, id: PageId) -> Result<Option<PageMeta>> {
        self.conn
            .query_row(
                "SELECT id, parent_id, title, icon, favorite, sort, updated_at FROM pages WHERE id = ?1",
                params![id],
                |row| {
                    Ok(PageMeta {
                        id: row.get(0)?,
                        parent: row.get(1)?,
                        title: row.get(2)?,
                        icon: row.get(3)?,
                        favorite: row.get::<_, i64>(4)? != 0,
                        sort: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn load_blocks(&self, page_id: PageId) -> Result<Vec<Block>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, text, checked, indent, lang, emoji FROM blocks
             WHERE page_id = ?1 ORDER BY ord ASC",
        )?;
        let rows = stmt.query_map(params![page_id], |row| {
            let kind = BlockKind::from_db(&row.get::<_, String>(0)?);
            Ok(Block {
                id: 0,
                kind,
                text: row.get(1)?,
                checked: row.get::<_, i64>(2)? != 0,
                indent: row.get::<_, i64>(3)?.clamp(0, 6) as u8,
                lang: row.get(4)?,
                emoji: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn load_page(&self, page_id: PageId) -> Result<Option<Page>> {
        let Some(meta) = self.page_meta(page_id)? else {
            return Ok(None);
        };
        let blocks = self.load_blocks(page_id)?;
        Ok(Some(Page { meta, blocks }))
    }

    pub fn save_page(&mut self, page: &Page) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE pages SET title = ?2, icon = ?3, updated_at = ?4 WHERE id = ?1",
            params![
                page.meta.id,
                page.meta.title,
                page.meta.icon,
                page.meta.updated_at
            ],
        )?;
        tx.execute(
            "DELETE FROM blocks WHERE page_id = ?1",
            params![page.meta.id],
        )?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO blocks (page_id, ord, kind, text, checked, indent, lang, emoji)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for (ord, block) in page.blocks.iter().enumerate() {
                stmt.execute(params![
                    page.meta.id,
                    ord as i64,
                    block.kind.as_str(),
                    block.text,
                    block.checked as i64,
                    block.indent as i64,
                    block.lang,
                    block.emoji,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn create_page(
        &mut self,
        parent: Option<PageId>,
        title: &str,
        icon: &str,
        now: i64,
    ) -> Result<PageMeta> {
        let sort: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(sort), 0) + 1 FROM pages WHERE parent_id IS ?1",
            params![parent],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO pages (parent_id, title, icon, favorite, sort, created_at, updated_at)
             VALUES (?1, ?2, ?3, 0, ?4, ?5, ?5)",
            params![parent, title, icon, sort, now],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(PageMeta {
            id,
            parent,
            title: title.to_owned(),
            icon: icon.to_owned(),
            favorite: false,
            sort,
            updated_at: now,
        })
    }

    pub fn set_favorite(&mut self, id: PageId, favorite: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE pages SET favorite = ?2 WHERE id = ?1",
            params![id, favorite as i64],
        )?;
        Ok(())
    }

    pub fn delete_page(&mut self, id: PageId) -> Result<()> {
        // ON DELETE CASCADE removes descendants, but only if the pragma is on.
        self.conn
            .execute("DELETE FROM pages WHERE id = ?1", params![id])?;
        // Remove any orphans defensively.
        self.conn.execute(
            "DELETE FROM pages WHERE parent_id IS NOT NULL
             AND parent_id NOT IN (SELECT id FROM pages)",
            [],
        )?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, 200) as i64;
        let mut hits: Vec<SearchHit> = Vec::new();
        let mut seen: std::collections::HashSet<PageId> = std::collections::HashSet::new();

        let folded_query = fold(query);

        // Title matches first.
        {
            let mut stmt = self.conn.prepare(
                "SELECT id, title, icon FROM pages
                 WHERE instr(fold(title), ?1) > 0
                 ORDER BY updated_at DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![folded_query, limit], |row| {
                Ok(SearchHit {
                    page_id: row.get(0)?,
                    title: row.get(1)?,
                    icon: row.get(2)?,
                    snippet: String::new(),
                })
            })?;
            for row in rows {
                let hit = row?;
                seen.insert(hit.page_id);
                hits.push(hit);
            }
        }

        // Then block text matches.
        if (hits.len() as i64) < limit {
            let mut stmt = self.conn.prepare(
                "SELECT p.id, p.title, p.icon, b.text
                 FROM blocks b JOIN pages p ON p.id = b.page_id
                 WHERE instr(fold(b.text), ?1) > 0
                 ORDER BY p.updated_at DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![folded_query, limit * 4], |row| {
                let text: String = row.get(3)?;
                Ok((
                    SearchHit {
                        page_id: row.get(0)?,
                        title: row.get(1)?,
                        icon: row.get(2)?,
                        snippet: snippet(&text, query),
                    },
                    text,
                ))
            })?;
            for row in rows {
                let (mut hit, text) = row?;
                if seen.contains(&hit.page_id) {
                    continue;
                }
                // Make sure the snippet actually contains the term.
                if hit.snippet.is_empty() {
                    hit.snippet = snippet(&text, query);
                }
                seen.insert(hit.page_id);
                hits.push(hit);
                if hits.len() as i64 >= limit {
                    break;
                }
            }
        }

        Ok(hits)
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        for lower in ch.to_lowercase() {
            out.push(match lower {
                'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'í' | 'ì' | 'î' | 'ï' => 'i',
                'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
                'ú' | 'ù' | 'û' | 'ü' => 'u',
                'ç' => 'c',
                'ñ' => 'n',
                other => other,
            });
        }
    }
    out
}

fn snippet(text: &str, query: &str) -> String {
    let haystack = fold(text);
    let needle = fold(query);
    let Some(byte_match) = haystack.find(&needle) else {
        return text.chars().take(90).collect();
    };
    let char_match = haystack[..byte_match].chars().count();
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    let start = char_match.saturating_sub(30);
    let end = (char_match + needle.chars().count() + 55).min(total);
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    for ch in &chars[start..end] {
        out.push(if *ch == '\n' { ' ' } else { *ch });
    }
    if end < total {
        out.push('…');
    }
    out
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SIMION_DATA_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("Simion");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Simion");
        }
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.trim().is_empty() {
            return PathBuf::from(xdg).join("simion");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("simion")
}

pub fn export_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
    for name in ["Documentos", "Documents"] {
        let candidate = PathBuf::from(&home).join(name);
        if candidate.is_dir() {
            return candidate.join("Simion");
        }
    }
    PathBuf::from(home).join("Simion")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::BlockKind;

    fn temp_store() -> (Store, PathBuf) {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "simion-test-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        (Store::open(&path).unwrap(), path)
    }

    #[test]
    fn seeds_and_reads_pages() {
        let (store, path) = temp_store();
        let pages = store.list_pages().unwrap();
        assert!(pages.len() >= 2);
        assert!(pages.iter().any(|p| p.title == "Primeiros passos"));
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn saves_and_loads_blocks() {
        let (mut store, path) = temp_store();
        let now = unix_now();
        let meta = store.create_page(None, "Teste", "🧪", now).unwrap();
        let page = Page {
            meta: meta.clone(),
            blocks: vec![
                Block::with_text(1, BlockKind::Heading1, "Olá"),
                Block::with_text(2, BlockKind::Paragraph, "Corpo com **unicornio**"),
                Block {
                    id: 3,
                    kind: BlockKind::Todo,
                    text: "feito".into(),
                    checked: true,
                    ..Default::default()
                },
            ],
        };
        store.save_page(&page).unwrap();
        let loaded = store.load_page(meta.id).unwrap().unwrap();
        assert_eq!(loaded.blocks.len(), 3);
        assert_eq!(loaded.blocks[0].kind, BlockKind::Heading1);
        assert!(loaded.blocks[2].checked);

        let hits = store.search("unicornio", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].page_id, meta.id);
        assert!(hits[0].snippet.contains("unicornio"));

        store.delete_page(meta.id).unwrap();
        assert!(store.load_page(meta.id).unwrap().is_none());
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn search_ignores_accents() {
        let (mut store, path) = temp_store();
        let now = unix_now();
        let meta = store.create_page(None, "Ação", "", now).unwrap();
        let page = Page {
            meta,
            blocks: vec![Block::with_text(
                1,
                BlockKind::Paragraph,
                "Uma citação com coração",
            )],
        };
        store.save_page(&page).unwrap();
        let hits = store.search("citacao", 10).unwrap();
        assert!(hits.iter().any(|hit| hit.snippet.contains("citação")));
        let hits = store.search("coração", 10).unwrap();
        assert!(!hits.is_empty());
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn deleting_parent_removes_children() {
        let (mut store, path) = temp_store();
        let now = unix_now();
        let parent = store.create_page(None, "Pai", "", now).unwrap();
        let child = store
            .create_page(Some(parent.id), "Filho", "", now)
            .unwrap();
        let grandchild = store.create_page(Some(child.id), "Neto", "", now).unwrap();
        store.delete_page(parent.id).unwrap();
        assert!(store.page_meta(parent.id).unwrap().is_none());
        assert!(store.page_meta(child.id).unwrap().is_none());
        assert!(store.page_meta(grandchild.id).unwrap().is_none());
        drop(store);
        let _ = std::fs::remove_file(path);
    }
}
