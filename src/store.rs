//! SQLite persistence. One local file, no server, no network.

use std::collections::HashSet;
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

CREATE TABLE IF NOT EXISTS assets (
    id          INTEGER PRIMARY KEY,
    page_id     INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    mime        TEXT NOT NULL,
    bytes       BLOB NOT NULL,
    width       INTEGER NOT NULL,
    height      INTEGER NOT NULL,
    created_at  INTEGER NOT NULL
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

/// Options shared by the search overlay and store callers.
#[derive(Clone, Copy, Debug, Default)]
pub struct SearchOptions {
    pub regex: bool,
    pub match_case: bool,
    pub whole_word: bool,
}

const MAX_MATCHES_PER_BLOCK: usize = 200;

struct SearchPattern {
    regex: regex::Regex,
    options: SearchOptions,
}

impl SearchPattern {
    fn new(query: &str, options: SearchOptions) -> Result<Self> {
        let pattern = if options.regex {
            // Validate before wrapping: a trailing backslash must remain an error.
            regex::RegexBuilder::new(query)
                .case_insensitive(!options.match_case)
                .unicode(true)
                .build()
                .map_err(|err| anyhow::anyhow!("Expressão regular inválida: {err}"))?;
            query.to_owned()
        } else {
            regex::escape(&search_fold(query, options.match_case))
        };
        let pattern = if options.whole_word {
            format!(r"\b(?:{pattern})\b")
        } else {
            pattern
        };
        let regex = regex::RegexBuilder::new(&pattern)
            .case_insensitive(options.regex && !options.match_case)
            .unicode(true)
            .build()
            .map_err(|err| anyhow::anyhow!("Expressão regular inválida: {err}"))?;
        Ok(Self { regex, options })
    }

    fn matches_in(&self, haystack: &str) -> Vec<std::ops::Range<usize>> {
        self.regex
            .find_iter(haystack)
            .take(MAX_MATCHES_PER_BLOCK)
            .map(|m| m.range())
            .collect()
    }

    #[cfg(test)]
    fn matches(&self, text: &str) -> Vec<std::ops::Range<usize>> {
        let folded;
        let haystack = if self.options.regex {
            text
        } else {
            folded = search_fold(text, self.options.match_case);
            &folded
        };
        self.matches_in(haystack)
    }

    fn first_match_chars(&self, text: &str) -> Option<(usize, usize)> {
        let folded;
        let haystack = if self.options.regex {
            text
        } else {
            folded = search_fold(text, self.options.match_case);
            &folded
        };
        let range = self.matches_in(haystack).into_iter().next()?;
        let start = haystack[..range.start].chars().count();
        let end = start + haystack[range.start..range.end].chars().count();
        Some((start, end))
    }
}

fn search_fold(text: &str, match_case: bool) -> String {
    if !match_case {
        return fold(text);
    }
    text.chars()
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'Ç' => 'C',
            'Ñ' => 'N',
            other => other,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub page_id: PageId,
    pub title: String,
    pub icon: String,
    pub snippet: String,
    pub block_ord: Option<i64>,
    pub match_chars: Option<(usize, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub mime: String,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
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
        let mut store = Self::open(&path)?;
        // GC so no boot: aqui nenhum snapshot de undo esta vivo. No save,
        // Ctrl+Z depois de apagar um bloco de imagem devolveria um id de asset
        // ja apagado. Falha no GC nao pode impedir o app de abrir.
        if let Err(err) = store.gc_orphan_assets() {
            eprintln!("simion: falha ao limpar assets orfaos: {err:#}");
        }
        Ok(store)
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

    /// Move `id` para ficar filha de `new_parent` (None = raiz), na posicao `index`
    /// (0 = primeiro) entre os irmaos de destino. `index` maior que o numero de
    /// irmaos e tratado como "no fim".
    pub fn move_page(
        &mut self,
        id: PageId,
        new_parent: Option<PageId>,
        index: usize,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        let Some(old_parent) = page_parent(&tx, id)? else {
            anyhow::bail!("pagina {id} nao existe");
        };

        // Anti-ciclo: o destino nao pode ser a propria pagina nem um descendente dela.
        if new_parent == Some(id) {
            anyhow::bail!("nao e possivel mover a pagina {id} para dentro dela mesma");
        }
        let mut cursor = new_parent;
        while let Some(current) = cursor {
            if current == id {
                anyhow::bail!("nao e possivel mover a pagina {id} para dentro de um descendente");
            }
            cursor = page_parent(&tx, current)?
                .ok_or_else(|| anyhow::anyhow!("pagina de destino {current} nao existe"))?;
        }

        // Irmaos de destino sem a pagina movida, com ela inserida na posicao pedida.
        let mut destination = sibling_ids(&tx, new_parent)?;
        destination.retain(|sibling| *sibling != id);
        let at = index.min(destination.len());
        destination.insert(at, id);

        if old_parent != new_parent {
            let mut origin = sibling_ids(&tx, old_parent)?;
            origin.retain(|sibling| *sibling != id);
            write_sort(&tx, &origin)?;
            tx.execute(
                "UPDATE pages SET parent_id = ?2 WHERE id = ?1",
                params![id, new_parent],
            )?;
        }
        write_sort(&tx, &destination)?;
        tx.commit()?;
        Ok(())
    }

    /// Caso particular de mesmo pai: reordena para `index`.
    pub fn reorder_page(&mut self, id: PageId, index: usize) -> Result<()> {
        let parent = page_parent(&self.conn, id)?
            .ok_or_else(|| anyhow::anyhow!("pagina {id} nao existe"))?;
        self.move_page(id, parent, index)
    }

    /// Sobe a pagina uma posicao entre as irmas. `false` quando ja e a primeira.
    pub fn move_page_up(&mut self, id: PageId) -> Result<bool> {
        let parent = page_parent(&self.conn, id)?
            .ok_or_else(|| anyhow::anyhow!("pagina {id} nao existe"))?;
        let siblings = sibling_ids(&self.conn, parent)?;
        let Some(index) = siblings.iter().position(|sibling| *sibling == id) else {
            anyhow::bail!("pagina {id} nao esta entre as irmas");
        };
        if index == 0 {
            return Ok(false);
        }
        self.reorder_page(id, index - 1)?;
        Ok(true)
    }

    /// Desce a pagina uma posicao entre as irmas. `false` quando ja e a ultima.
    pub fn move_page_down(&mut self, id: PageId) -> Result<bool> {
        let parent = page_parent(&self.conn, id)?
            .ok_or_else(|| anyhow::anyhow!("pagina {id} nao existe"))?;
        let siblings = sibling_ids(&self.conn, parent)?;
        let Some(index) = siblings.iter().position(|sibling| *sibling == id) else {
            anyhow::bail!("pagina {id} nao esta entre as irmas");
        };
        if index + 1 >= siblings.len() {
            return Ok(false);
        }
        self.reorder_page(id, index + 1)?;
        Ok(true)
    }

    /// Reparenta para a irma anterior, virando a ultima filha dela.
    /// `false` quando a pagina e a primeira entre as irmas.
    pub fn indent_page(&mut self, id: PageId) -> Result<bool> {
        let parent = page_parent(&self.conn, id)?
            .ok_or_else(|| anyhow::anyhow!("pagina {id} nao existe"))?;
        let siblings = sibling_ids(&self.conn, parent)?;
        let Some(index) = siblings.iter().position(|sibling| *sibling == id) else {
            anyhow::bail!("pagina {id} nao esta entre as irmas");
        };
        if index == 0 {
            return Ok(false);
        }
        let new_parent = siblings[index - 1];
        let at = sibling_ids(&self.conn, Some(new_parent))?.len();
        self.move_page(id, Some(new_parent), at)?;
        Ok(true)
    }

    /// Sai do pai e vira irma dele, logo depois. `false` quando ja e raiz.
    pub fn outdent_page(&mut self, id: PageId) -> Result<bool> {
        let parent = page_parent(&self.conn, id)?
            .ok_or_else(|| anyhow::anyhow!("pagina {id} nao existe"))?;
        let Some(parent) = parent else {
            return Ok(false);
        };
        let grandparent = page_parent(&self.conn, parent)?
            .ok_or_else(|| anyhow::anyhow!("pagina {parent} nao existe"))?;
        let siblings = sibling_ids(&self.conn, grandparent)?;
        let Some(index) = siblings.iter().position(|sibling| *sibling == parent) else {
            anyhow::bail!("pagina {parent} nao esta entre as irmas");
        };
        self.move_page(id, grandparent, index + 1)?;
        Ok(true)
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
        self.search_with(query, SearchOptions::default(), limit)
    }

    pub fn search_with(
        &self,
        query: &str,
        options: SearchOptions,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let query = query.trim();
        let pattern = SearchPattern::new(query, options)?;
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, 200);
        let mut hits = Vec::new();
        let mut seen = HashSet::new();
        // Titles retain priority over block matches, with one hit per page.
        for titles in [true, false] {
            let sql = if titles {
                "SELECT id, title, icon, title, NULL FROM pages ORDER BY updated_at DESC"
            } else {
                "SELECT p.id, p.title, p.icon, b.text, b.ord
                 FROM blocks b JOIN pages p ON p.id = b.page_id
                 WHERE b.kind != 'image' ORDER BY p.updated_at DESC, b.ord ASC"
            };
            let mut stmt = self.conn.prepare(sql)?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            })?;
            for row in rows {
                let (page_id, title, icon, text, block_ord) = row?;
                if seen.contains(&page_id) {
                    continue;
                }
                let Some(match_chars) = pattern.first_match_chars(&text) else {
                    continue;
                };
                seen.insert(page_id);
                hits.push(SearchHit {
                    page_id,
                    title,
                    icon,
                    snippet: if titles {
                        String::new()
                    } else {
                        snippet(&text, query)
                    },
                    block_ord,
                    match_chars: Some(match_chars),
                });
                if hits.len() >= limit {
                    return Ok(hits);
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

    pub fn block_count(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM blocks", [], |row| row.get(0))
            .map_err(Into::into)
    }

    pub fn insert_asset(
        &self,
        page_id: PageId,
        mime: &str,
        bytes: &[u8],
        width: u32,
        height: u32,
    ) -> Result<u64> {
        self.conn.execute(
            "INSERT INTO assets (page_id, mime, bytes, width, height, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                page_id,
                mime,
                bytes,
                width as i64,
                height as i64,
                unix_now()
            ],
        )?;
        Ok(self.conn.last_insert_rowid() as u64)
    }

    pub fn asset(&self, id: u64) -> Result<Option<Asset>> {
        self.conn
            .query_row(
                "SELECT mime, bytes, width, height FROM assets WHERE id = ?1",
                params![id as i64],
                |row| {
                    Ok(Asset {
                        mime: row.get(0)?,
                        bytes: row.get(1)?,
                        width: row.get::<_, i64>(2)?.clamp(0, u32::MAX as i64) as u32,
                        height: row.get::<_, i64>(3)?.clamp(0, u32::MAX as i64) as u32,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Copies an asset to another page (used when duplicating a page, so the
    /// copy keeps its images after the original is deleted). `None` when the
    /// source asset does not exist.
    pub fn copy_asset(&self, id: u64, to_page: PageId) -> Result<Option<u64>> {
        let copied = self.conn.execute(
            "INSERT INTO assets (page_id, mime, bytes, width, height, created_at)
             SELECT ?1, mime, bytes, width, height, ?2 FROM assets WHERE id = ?3",
            params![to_page, unix_now(), id as i64],
        )?;
        Ok((copied == 1).then(|| self.conn.last_insert_rowid() as u64))
    }

    /// Apaga os assets que nenhum bloco `image` (de qualquer pagina)
    /// referencia. O id e lido com `parse::<u64>()`, igual ao editor, para que
    /// texto vazio ou nao numerico nunca proteja um asset. Retorna quantos
    /// foram apagados. So deve rodar no boot (ver `open_default`).
    pub fn gc_orphan_assets(&mut self) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let referenced: HashSet<u64> = {
            let mut stmt = tx.prepare("SELECT text FROM blocks WHERE kind = 'image'")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            let mut ids = HashSet::new();
            for text in rows {
                if let Ok(id) = text?.parse::<u64>() {
                    ids.insert(id);
                }
            }
            ids
        };
        let orphans: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT id FROM assets")?;
            let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
            let mut out = Vec::new();
            for id in rows {
                let id = id?;
                if !u64::try_from(id).is_ok_and(|id| referenced.contains(&id)) {
                    out.push(id);
                }
            }
            out
        };
        {
            let mut stmt = tx.prepare("DELETE FROM assets WHERE id = ?1")?;
            for id in &orphans {
                stmt.execute(params![id])?;
            }
        }
        tx.commit()?;
        Ok(orphans.len())
    }

    pub fn page_asset_sizes(&self, page_id: PageId) -> Result<Vec<(u64, u32, u32)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, width, height FROM assets WHERE page_id = ?1 ORDER BY id ASC")?;
        let rows = stmt.query_map(params![page_id], |row| {
            let id = row.get::<_, i64>(0)?.max(0) as u64;
            let width = row.get::<_, i64>(1)?.clamp(0, u32::MAX as i64) as u32;
            let height = row.get::<_, i64>(2)?.clamp(0, u32::MAX as i64) as u32;
            Ok((id, width, height))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

/// Pai da pagina: `None` quando a pagina nao existe, `Some(None)` quando e raiz.
fn page_parent(conn: &Connection, id: PageId) -> Result<Option<Option<PageId>>> {
    conn.query_row(
        "SELECT parent_id FROM pages WHERE id = ?1",
        params![id],
        |row| row.get::<_, Option<PageId>>(0),
    )
    .optional()
    .map_err(Into::into)
}

/// Filhos do pai, na ordem gravada.
fn sibling_ids(conn: &Connection, parent: Option<PageId>) -> Result<Vec<PageId>> {
    let mut stmt =
        conn.prepare("SELECT id FROM pages WHERE parent_id IS ?1 ORDER BY sort ASC, id ASC")?;
    let rows = stmt.query_map(params![parent], |row| row.get(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Reescreve `sort` como 0,1,2,... na ordem recebida.
fn write_sort(conn: &Connection, ids: &[PageId]) -> Result<()> {
    let mut stmt = conn.prepare("UPDATE pages SET sort = ?2 WHERE id = ?1")?;
    for (sort, id) in ids.iter().enumerate() {
        stmt.execute(params![id, sort as i64])?;
    }
    Ok(())
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

    /// Banco vazio, sem as paginas iniciais, para testar ordem de raiz.
    fn blank_store() -> (Store, PathBuf) {
        let (mut store, path) = temp_store();
        for page in store.list_pages().unwrap() {
            store.delete_page(page.id).unwrap();
        }
        assert!(store.list_pages().unwrap().is_empty());
        (store, path)
    }

    fn child_ids(store: &Store, parent: Option<PageId>) -> Vec<PageId> {
        let mut pages = store.list_pages().unwrap();
        pages.retain(|page| page.parent == parent);
        pages.sort_by_key(|page| page.sort);
        pages.into_iter().map(|page| page.id).collect()
    }

    fn child_sorts(store: &Store, parent: Option<PageId>) -> Vec<i64> {
        let mut pages = store.list_pages().unwrap();
        pages.retain(|page| page.parent == parent);
        pages.sort_by_key(|page| page.sort);
        pages.into_iter().map(|page| page.sort).collect()
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
    fn search_options_combine_on_titles_and_blocks() {
        let (mut store, path) = temp_store();
        store.conn.execute("DELETE FROM pages", []).unwrap();
        let meta = store.create_page(None, "Árvore", "", unix_now()).unwrap();
        store
            .save_page(&Page {
                meta,
                blocks: vec![Block::with_text(1, BlockKind::Paragraph, "Pré ação pós")],
            })
            .unwrap();
        let default = SearchOptions::default();
        let sensitive = SearchOptions {
            match_case: true,
            ..default
        };
        assert_eq!(store.search_with("arvore", default, 10).unwrap().len(), 1);
        assert!(
            store
                .search_with("arvore", sensitive, 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.search_with("Arvore", sensitive, 10).unwrap().len(), 1);
        assert_eq!(store.search_with("pre", default, 10).unwrap().len(), 1);
        assert!(store.search_with("pre", sensitive, 10).unwrap().is_empty());
        assert_eq!(store.search_with("Pre", sensitive, 10).unwrap().len(), 1);
        let word = SearchOptions {
            whole_word: true,
            ..default
        };
        assert_eq!(store.search_with("acao", word, 10).unwrap().len(), 1);
        assert!(store.search_with("cao", word, 10).unwrap().is_empty());
        assert_eq!(store.search_with("cao", default, 10).unwrap().len(), 1);
        let regex = SearchOptions {
            regex: true,
            whole_word: true,
            ..default
        };
        assert_eq!(store.search_with("[aàâ]ção", regex, 10).unwrap().len(), 1);
        assert!(store.search_with("acao", regex, 10).unwrap().is_empty());
        assert_eq!(store.search_with("árvore", regex, 10).unwrap().len(), 1);
        assert!(
            store
                .search_with(
                    "árvore",
                    SearchOptions {
                        match_case: true,
                        ..regex
                    },
                    10
                )
                .unwrap()
                .is_empty()
        );
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn whole_word_uses_unicode_boundaries() {
        for regex in [false, true] {
            let options = SearchOptions {
                regex,
                whole_word: true,
                ..Default::default()
            };
            let pattern = SearchPattern::new("猫", options).unwrap();
            assert!(pattern.matches("黑猫").is_empty());
            assert_eq!(pattern.matches("猫! 猫").len(), 2);
        }
    }

    #[test]
    fn invalid_regex_returns_error() {
        let (store, path) = temp_store();
        for whole_word in [false, true] {
            for query in ["[", "(", "abc\\"] {
                let error = store
                    .search_with(
                        query,
                        SearchOptions {
                            regex: true,
                            whole_word,
                            ..Default::default()
                        },
                        10,
                    )
                    .unwrap_err();
                assert!(error.to_string().contains("Expressão regular inválida"));
            }
        }
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn zero_length_regex_matches_are_bounded() {
        let options = SearchOptions {
            regex: true,
            ..Default::default()
        };
        let text = "a ".repeat(1000);
        for query in [r"\b", "", "a?"] {
            let matches = SearchPattern::new(query, options).unwrap().matches(&text);
            assert_eq!(matches.len(), MAX_MATCHES_PER_BLOCK);
        }
        assert_eq!(
            SearchPattern::new("^", options).unwrap().matches(&text),
            vec![0..0]
        );
        let (mut store, path) = temp_store();
        store.conn.execute("DELETE FROM pages", []).unwrap();
        store.create_page(None, &text, "", unix_now()).unwrap();
        let meta = store.create_page(None, "", "", unix_now()).unwrap();
        store
            .save_page(&Page {
                meta,
                blocks: vec![Block::with_text(1, BlockKind::Paragraph, &text)],
            })
            .unwrap();
        assert_eq!(store.search_with(r"\b", options, 10).unwrap().len(), 2);
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
    fn search_with_reports_block_ord_and_char_range_with_accents() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let title = "Resumo da Ação";
        let meta = store.create_page(None, title, "", now).unwrap();
        let block0 = "Primeiro bloco sem o termo";
        let block1 = "Uma citação com coração";
        let block2 = "🌱 Pré ação pós coração";
        store
            .save_page(&Page {
                meta: meta.clone(),
                blocks: vec![
                    Block::with_text(1, BlockKind::Paragraph, block0),
                    Block::with_text(2, BlockKind::Paragraph, block1),
                    Block::with_text(3, BlockKind::Paragraph, block2),
                ],
            })
            .unwrap();

        let slice_chars = |text: &str, (start, end): (usize, usize)| -> String {
            text.chars().skip(start).take(end - start).collect()
        };

        // Match no título sem regex (dobrando acentos).
        let hits = store
            .search_with("acao", SearchOptions::default(), 10)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].page_id, meta.id);
        assert_eq!(hits[0].block_ord, None);
        assert_eq!(hits[0].match_chars, Some((10, 14)));
        assert_eq!(slice_chars(title, hits[0].match_chars.unwrap()), "Ação");

        // Match no bloco 1 sem regex (dobrando acentos).
        let hits = store
            .search_with("citacao", SearchOptions::default(), 10)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_ord, Some(1));
        assert_eq!(hits[0].match_chars, Some((4, 11)));
        assert_eq!(slice_chars(block1, hits[0].match_chars.unwrap()), "citação");

        // Quando vários blocos casam, pega o primeiro em ordem de `ord`.
        let hits = store
            .search_with("coracao", SearchOptions::default(), 10)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_ord, Some(1));
        assert_eq!(slice_chars(block1, hits[0].match_chars.unwrap()), "coração");

        // Match com emoji multibyte antes do termo (folded e regex).
        let hits = store
            .search_with("pre", SearchOptions::default(), 10)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_ord, Some(2));
        assert_eq!(slice_chars(block2, hits[0].match_chars.unwrap()), "Pré");

        let regex_opts = SearchOptions {
            regex: true,
            ..Default::default()
        };
        let hits = store.search_with("[aà]ção", regex_opts, 10).unwrap();
        assert_eq!(hits.len(), 1);
        // Título tem prioridade sobre blocos.
        assert_eq!(hits[0].block_ord, None);
        assert_eq!(slice_chars(title, hits[0].match_chars.unwrap()), "Ação");

        let hits = store.search_with("Pré\\s+ação", regex_opts, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_ord, Some(2));
        assert_eq!(
            slice_chars(block2, hits[0].match_chars.unwrap()),
            "Pré ação"
        );

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

    #[test]
    fn auto_update_setting_round_trips() {
        use crate::app::{auto_update_value, parse_auto_update};
        let (mut store, path) = temp_store();
        let key = "auto_update_check";
        assert_eq!(store.setting(key).unwrap(), None);
        assert!(parse_auto_update(store.setting(key).unwrap().as_deref()));

        for _ in 0..2 {
            store.set_setting(key, auto_update_value(false)).unwrap();
            assert_eq!(store.setting(key).unwrap().as_deref(), Some("0"));
            assert!(!parse_auto_update(store.setting(key).unwrap().as_deref()));
        }
        for _ in 0..2 {
            store.set_setting(key, auto_update_value(true)).unwrap();
            assert_eq!(store.setting(key).unwrap().as_deref(), Some("1"));
            assert!(parse_auto_update(store.setting(key).unwrap().as_deref()));
        }
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn counts_blocks() {
        let (store, path) = temp_store();
        assert!(store.block_count().unwrap() > 0);
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn moves_page_into_the_middle_of_another_parent() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let origem = store.create_page(None, "Origem", "", now).unwrap();
        let destino = store.create_page(None, "Destino", "", now).unwrap();
        let a = store.create_page(Some(origem.id), "A", "", now).unwrap();
        let b = store.create_page(Some(origem.id), "B", "", now).unwrap();
        let c = store.create_page(Some(origem.id), "C", "", now).unwrap();
        let d = store.create_page(Some(destino.id), "D", "", now).unwrap();
        let e = store.create_page(Some(destino.id), "E", "", now).unwrap();

        store.move_page(b.id, Some(destino.id), 1).unwrap();

        assert_eq!(child_ids(&store, Some(origem.id)), vec![a.id, c.id]);
        assert_eq!(child_ids(&store, Some(destino.id)), vec![d.id, b.id, e.id]);
        assert_eq!(child_sorts(&store, Some(origem.id)), vec![0, 1]);
        assert_eq!(child_sorts(&store, Some(destino.id)), vec![0, 1, 2]);
        assert_eq!(
            store.page_meta(b.id).unwrap().unwrap().parent,
            Some(destino.id)
        );
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn moves_page_to_root_at_the_end() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let raiz = store.create_page(None, "Raiz", "", now).unwrap();
        let outra = store.create_page(None, "Outra", "", now).unwrap();
        let filha = store.create_page(Some(raiz.id), "Filha", "", now).unwrap();

        store.move_page(filha.id, None, 99).unwrap();

        assert!(child_ids(&store, Some(raiz.id)).is_empty());
        assert_eq!(child_ids(&store, None), vec![raiz.id, outra.id, filha.id]);
        assert_eq!(child_sorts(&store, None), vec![0, 1, 2]);
        assert_eq!(store.page_meta(filha.id).unwrap().unwrap().parent, None);
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reorders_siblings_up_and_down() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let pai = store.create_page(None, "Pai", "", now).unwrap();
        let a = store.create_page(Some(pai.id), "A", "", now).unwrap();
        let b = store.create_page(Some(pai.id), "B", "", now).unwrap();
        let c = store.create_page(Some(pai.id), "C", "", now).unwrap();

        store.reorder_page(c.id, 0).unwrap();
        assert_eq!(child_ids(&store, Some(pai.id)), vec![c.id, a.id, b.id]);

        store.reorder_page(c.id, 1).unwrap();
        assert_eq!(child_ids(&store, Some(pai.id)), vec![a.id, c.id, b.id]);

        store.reorder_page(a.id, 99).unwrap();
        assert_eq!(child_ids(&store, Some(pai.id)), vec![c.id, b.id, a.id]);

        // Indice atual e no-op idempotente.
        store.reorder_page(a.id, 2).unwrap();
        assert_eq!(child_ids(&store, Some(pai.id)), vec![c.id, b.id, a.id]);
        assert_eq!(child_sorts(&store, Some(pai.id)), vec![0, 1, 2]);
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_moving_page_into_a_descendant() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let pai = store.create_page(None, "Pai", "", now).unwrap();
        let filha = store.create_page(Some(pai.id), "Filha", "", now).unwrap();
        let neta = store.create_page(Some(filha.id), "Neta", "", now).unwrap();
        let irma = store.create_page(None, "Irma", "", now).unwrap();

        let raiz_antes = child_ids(&store, None);
        let pai_antes = child_ids(&store, Some(pai.id));
        let filha_antes = child_ids(&store, Some(filha.id));

        assert!(store.move_page(pai.id, Some(filha.id), 0).is_err());
        assert!(store.move_page(pai.id, Some(neta.id), 0).is_err());
        assert!(store.move_page(pai.id, Some(pai.id), 0).is_err());

        // Estado intacto depois das rejeicoes.
        assert_eq!(child_ids(&store, None), raiz_antes);
        assert_eq!(child_ids(&store, Some(pai.id)), pai_antes);
        assert_eq!(child_ids(&store, Some(filha.id)), filha_antes);
        assert_eq!(store.page_meta(pai.id).unwrap().unwrap().parent, None);
        assert!(child_ids(&store, None).contains(&irma.id));
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn moves_page_one_step_up_and_down() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let pai = store.create_page(None, "Pai", "", now).unwrap();
        let a = store.create_page(Some(pai.id), "A", "", now).unwrap();
        let b = store.create_page(Some(pai.id), "B", "", now).unwrap();
        let c = store.create_page(Some(pai.id), "C", "", now).unwrap();

        assert!(store.move_page_down(a.id).unwrap());
        assert_eq!(child_ids(&store, Some(pai.id)), vec![b.id, a.id, c.id]);
        assert!(store.move_page_up(c.id).unwrap());
        assert_eq!(child_ids(&store, Some(pai.id)), vec![b.id, c.id, a.id]);

        assert!(!store.move_page_up(b.id).unwrap());
        assert!(!store.move_page_down(a.id).unwrap());
        assert_eq!(child_ids(&store, Some(pai.id)), vec![b.id, c.id, a.id]);
        assert_eq!(child_sorts(&store, Some(pai.id)), vec![0, 1, 2]);
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn indents_page_under_the_previous_sibling() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let pai = store.create_page(None, "Pai", "", now).unwrap();
        let a = store.create_page(Some(pai.id), "A", "", now).unwrap();
        let b = store.create_page(Some(pai.id), "B", "", now).unwrap();
        let c = store.create_page(Some(pai.id), "C", "", now).unwrap();
        let neto = store.create_page(Some(b.id), "Neto", "", now).unwrap();

        assert!(store.indent_page(c.id).unwrap());
        assert_eq!(child_ids(&store, Some(pai.id)), vec![a.id, b.id]);
        assert_eq!(child_ids(&store, Some(b.id)), vec![neto.id, c.id]);
        assert_eq!(child_sorts(&store, Some(b.id)), vec![0, 1]);
        assert_eq!(store.page_meta(c.id).unwrap().unwrap().parent, Some(b.id));

        // Primeira irma nao tem para onde indentar.
        assert!(!store.indent_page(a.id).unwrap());
        assert_eq!(child_ids(&store, Some(pai.id)), vec![a.id, b.id]);
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn outdents_page_after_its_parent() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let pai = store.create_page(None, "Pai", "", now).unwrap();
        let irma = store.create_page(None, "Irma", "", now).unwrap();
        let a = store.create_page(Some(pai.id), "A", "", now).unwrap();
        let b = store.create_page(Some(pai.id), "B", "", now).unwrap();
        let neto = store.create_page(Some(a.id), "Neto", "", now).unwrap();

        assert!(store.outdent_page(neto.id).unwrap());
        assert_eq!(child_ids(&store, Some(pai.id)), vec![a.id, neto.id, b.id]);
        assert!(child_ids(&store, Some(a.id)).is_empty());
        assert_eq!(child_sorts(&store, Some(pai.id)), vec![0, 1, 2]);

        // Raiz nao tem avo: gesto idempotente, nada muda.
        let raizes = child_ids(&store, None);
        assert!(!store.outdent_page(pai.id).unwrap());
        assert_eq!(child_ids(&store, None), raizes);
        assert_eq!(child_ids(&store, None), vec![pai.id, irma.id]);
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn indenting_a_page_can_never_create_a_cycle() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let pai = store.create_page(None, "Pai", "", now).unwrap();
        let filha = store.create_page(Some(pai.id), "Filha", "", now).unwrap();
        let irma = store.create_page(Some(pai.id), "Irma", "", now).unwrap();

        // Indentar a irma sob a filha e valido; mover o pai para dentro da
        // filha continua barrado por `move_page`.
        assert!(store.indent_page(irma.id).unwrap());
        assert_eq!(
            store.page_meta(irma.id).unwrap().unwrap().parent,
            Some(filha.id)
        );
        assert!(store.move_page(pai.id, Some(filha.id), 0).is_err());
        assert_eq!(store.page_meta(pai.id).unwrap().unwrap().parent, None);
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_unknown_page() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let pai = store.create_page(None, "Pai", "", now).unwrap();
        let filha = store.create_page(Some(pai.id), "Filha", "", now).unwrap();

        let raiz_antes = child_ids(&store, None);
        assert!(store.move_page(9999, Some(pai.id), 0).is_err());
        assert!(store.move_page(9999, None, 0).is_err());
        assert!(store.reorder_page(9999, 0).is_err());
        assert!(store.move_page(filha.id, Some(9999), 0).is_err());
        assert_eq!(child_ids(&store, None), raiz_antes);
        assert_eq!(
            store.page_meta(filha.id).unwrap().unwrap().parent,
            Some(pai.id)
        );
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn normalizes_sort_after_moves() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let pai = store.create_page(None, "Pai", "", now).unwrap();
        let a = store.create_page(Some(pai.id), "A", "", now).unwrap();
        let b = store.create_page(Some(pai.id), "B", "", now).unwrap();
        let c = store.create_page(Some(pai.id), "C", "", now).unwrap();
        let d = store.create_page(Some(pai.id), "D", "", now).unwrap();
        let e = store.create_page(Some(pai.id), "E", "", now).unwrap();

        store.move_page(d.id, None, 1).unwrap();
        store.move_page(e.id, Some(pai.id), 0).unwrap();
        store.move_page(a.id, Some(pai.id), 2).unwrap();
        store.move_page(b.id, Some(pai.id), 99).unwrap();

        assert_eq!(
            child_ids(&store, Some(pai.id)),
            vec![e.id, a.id, c.id, b.id]
        );
        for parent in [None, Some(pai.id)] {
            let sorts = child_sorts(&store, parent);
            let esperado: Vec<i64> = (0..sorts.len() as i64).collect();
            assert_eq!(sorts, esperado);
            let unicos: std::collections::HashSet<i64> = sorts.iter().copied().collect();
            assert_eq!(unicos.len(), sorts.len());
        }
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn create_page_appends_after_moves() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let pai = store.create_page(None, "Pai", "", now).unwrap();
        let a = store.create_page(Some(pai.id), "A", "", now).unwrap();
        let b = store.create_page(Some(pai.id), "B", "", now).unwrap();
        let c = store.create_page(Some(pai.id), "C", "", now).unwrap();

        store.reorder_page(c.id, 0).unwrap();
        store.move_page(a.id, None, 0).unwrap();

        let d = store.create_page(Some(pai.id), "D", "", now).unwrap();
        assert_eq!(d.sort, 2);
        let e = store.create_page(None, "E", "", now).unwrap();
        assert_eq!(e.sort, 2);
        assert_eq!(child_ids(&store, Some(pai.id)), vec![c.id, b.id, d.id]);
        assert_eq!(child_sorts(&store, Some(pai.id)), vec![0, 1, 2]);
        assert_eq!(child_ids(&store, None).last().copied(), Some(e.id));
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn inserts_and_reads_assets() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let meta = store.create_page(None, "Com imagem", "", now).unwrap();
        let bytes = vec![0x89, b'P', b'N', b'G', 1, 2, 3, 4];
        let asset_id = store
            .insert_asset(meta.id, "image/png", &bytes, 640, 480)
            .unwrap();

        let asset = store.asset(asset_id).unwrap().unwrap();
        assert_eq!(asset.mime, "image/png");
        assert_eq!(asset.bytes, bytes);
        assert_eq!((asset.width, asset.height), (640, 480));
        assert_eq!(
            store.page_asset_sizes(meta.id).unwrap(),
            vec![(asset_id, 640, 480)]
        );

        let page = Page {
            meta,
            blocks: vec![Block::with_text(1, BlockKind::Image, asset_id.to_string())],
        };
        store.save_page(&page).unwrap();
        assert!(store.search(&asset_id.to_string(), 10).unwrap().is_empty());

        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn copied_asset_survives_deleting_the_source_page() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let source = store.create_page(None, "Origem", "", now).unwrap();
        let copy = store.create_page(None, "Cópia", "", now).unwrap();
        let original = store
            .insert_asset(source.id, "image/png", &[7, 8, 9], 3, 1)
            .unwrap();

        let copied = store.copy_asset(original, copy.id).unwrap().unwrap();
        assert_ne!(copied, original);
        assert_eq!(store.copy_asset(999_999, copy.id).unwrap(), None);

        store.delete_page(source.id).unwrap();
        assert!(store.asset(original).unwrap().is_none());
        let asset = store.asset(copied).unwrap().unwrap();
        assert_eq!(asset.bytes, vec![7, 8, 9]);
        assert_eq!((asset.width, asset.height), (3, 1));

        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn page_asset_sizes_only_lists_the_requested_page() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let first = store.create_page(None, "Primeira", "", now).unwrap();
        let second = store.create_page(None, "Segunda", "", now).unwrap();
        let first_asset = store
            .insert_asset(first.id, "image/png", &[1, 2, 3], 10, 20)
            .unwrap();
        let second_asset = store
            .insert_asset(second.id, "image/png", &[4, 5, 6], 30, 40)
            .unwrap();

        assert_eq!(
            store.page_asset_sizes(first.id).unwrap(),
            vec![(first_asset, 10, 20)]
        );
        assert_eq!(
            store.page_asset_sizes(second.id).unwrap(),
            vec![(second_asset, 30, 40)]
        );

        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn deleting_a_page_cascades_to_its_assets_only() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let gone = store.create_page(None, "Some", "", now).unwrap();
        let kept = store.create_page(None, "Fica", "", now).unwrap();
        let gone_asset = store
            .insert_asset(gone.id, "image/png", &[1, 2, 3], 1, 2)
            .unwrap();
        let kept_asset = store
            .insert_asset(kept.id, "image/png", &[4, 5, 6], 3, 4)
            .unwrap();

        store.delete_page(gone.id).unwrap();

        assert!(store.asset(gone_asset).unwrap().is_none());
        assert!(store.page_asset_sizes(gone.id).unwrap().is_empty());
        let asset = store.asset(kept_asset).unwrap().unwrap();
        assert_eq!(asset.bytes, vec![4, 5, 6]);
        assert_eq!(
            store.page_asset_sizes(kept.id).unwrap(),
            vec![(kept_asset, 3, 4)]
        );

        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn gc_removes_orphans_and_keeps_referenced_assets() {
        let (mut store, path) = blank_store();
        let now = unix_now();
        let first = store.create_page(None, "Primeira", "", now).unwrap();
        let second = store.create_page(None, "Segunda", "", now).unwrap();
        let same_page = store
            .insert_asset(first.id, "image/png", &[1], 1, 1)
            .unwrap();
        let orphan = store
            .insert_asset(first.id, "image/png", &[2], 1, 1)
            .unwrap();
        // Asset de uma pagina referenciado por bloco de outra: o GC e global.
        let cross_page = store
            .insert_asset(second.id, "image/png", &[3], 1, 1)
            .unwrap();
        // Paragrafo com o id como texto nao protege; texto vazio e lixo tambem nao.
        let only_paragraph = store
            .insert_asset(second.id, "image/png", &[4], 1, 1)
            .unwrap();

        let first_page = Page {
            meta: first,
            blocks: vec![
                Block::with_text(1, BlockKind::Image, same_page.to_string()),
                Block::with_text(2, BlockKind::Image, cross_page.to_string()),
                Block::with_text(3, BlockKind::Image, ""),
                Block::with_text(4, BlockKind::Image, format!("{only_paragraph}abc")),
            ],
        };
        store.save_page(&first_page).unwrap();
        let paragraph = Block::with_text(1, BlockKind::Paragraph, only_paragraph.to_string());
        let second_page = Page {
            meta: second,
            blocks: vec![paragraph],
        };
        store.save_page(&second_page).unwrap();

        assert_eq!(store.gc_orphan_assets().unwrap(), 2);
        assert!(store.asset(orphan).unwrap().is_none());
        assert!(store.asset(only_paragraph).unwrap().is_none());
        assert!(store.asset(same_page).unwrap().is_some());
        assert!(store.asset(cross_page).unwrap().is_some());
        // Idempotente.
        assert_eq!(store.gc_orphan_assets().unwrap(), 0);

        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn gc_with_no_assets_returns_zero() {
        let (mut store, path) = temp_store();
        assert_eq!(store.gc_orphan_assets().unwrap(), 0);
        let (mut blank, blank_path) = blank_store();
        assert_eq!(blank.gc_orphan_assets().unwrap(), 0);

        drop(store);
        drop(blank);
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(blank_path);
    }
}
