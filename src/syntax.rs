//! Code block languages and syntax coloring in the app palette.

use std::collections::{HashMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};

use eframe::egui::Color32;
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

use crate::theme::{Mode, Palette, palette};

pub type Spans = Arc<Vec<(Range<usize>, Color32)>>;

/// Languages offered by the picker: (id, label). `plain` means no coloring.
pub const LANGS: &[(&str, &str)] = &[
    ("plain", "Texto"),
    ("bash", "Bash"),
    ("c", "C"),
    ("c++", "C++"),
    ("c#", "C#"),
    ("css", "CSS"),
    ("dart", "Dart"),
    ("docker", "Docker"),
    ("elixir", "Elixir"),
    ("erlang", "Erlang"),
    ("go", "Go"),
    ("graphql", "GraphQL"),
    ("groovy", "Groovy"),
    ("haskell", "Haskell"),
    ("html", "HTML"),
    ("java", "Java"),
    ("javascript", "JavaScript"),
    ("json", "JSON"),
    ("kotlin", "Kotlin"),
    ("latex", "LaTeX"),
    ("less", "Less"),
    ("lua", "Lua"),
    ("makefile", "Makefile"),
    ("markdown", "Markdown"),
    ("matlab", "MATLAB"),
    ("objective-c", "Objective-C"),
    ("perl", "Perl"),
    ("php", "PHP"),
    ("powershell", "PowerShell"),
    ("python", "Python"),
    ("r", "R"),
    ("ruby", "Ruby"),
    ("rust", "Rust"),
    ("scss", "Sass/SCSS"),
    ("scala", "Scala"),
    ("sql", "SQL"),
    ("swift", "Swift"),
    ("toml", "TOML"),
    ("typescript", "TypeScript"),
    ("xml", "XML"),
    ("yaml", "YAML"),
];

/// Resolves a language name or alias to an id from `LANGS` (`plain` when unknown).
pub fn normalize(lang: &str) -> &'static str {
    let lower = lang.trim().to_lowercase();
    if let Some((id, _)) = LANGS.iter().find(|(id, _)| *id == lower) {
        return id;
    }
    match lower.as_str() {
        "js" | "jsx" | "mjs" | "cjs" | "node" => "javascript",
        "ts" | "tsx" => "typescript",
        "py" | "python3" => "python",
        "rs" => "rust",
        "sh" | "shell" | "zsh" | "console" | "fish" => "bash",
        "yml" => "yaml",
        "cpp" | "cxx" | "cc" | "hpp" => "c++",
        "cs" | "csharp" => "c#",
        "objc" | "objectivec" | "obj-c" => "objective-c",
        "md" => "markdown",
        "ps" | "ps1" | "pwsh" => "powershell",
        "rb" => "ruby",
        "sass" => "scss",
        "dockerfile" => "docker",
        "golang" => "go",
        "kt" | "kts" => "kotlin",
        "ex" | "exs" => "elixir",
        "erl" => "erlang",
        "hs" => "haskell",
        "tex" => "latex",
        "make" | "mk" => "makefile",
        "htm" | "xhtml" => "html",
        "gql" => "graphql",
        "pl" => "perl",
        _ => "plain",
    }
}

/// Value stored in `Block::lang`: a known id, `""` for plain text, or the
/// lowercased name of an unknown language (kept for Markdown round-trips).
pub fn canonical(lang: &str) -> String {
    match normalize(lang) {
        "plain" => {
            let lower = lang.trim().to_lowercase();
            if matches!(lower.as_str(), "" | "plain" | "text" | "txt" | "plaintext") {
                String::new()
            } else {
                lower
            }
        }
        id => id.to_owned(),
    }
}

pub fn label(lang: &str) -> &'static str {
    let id = normalize(lang);
    LANGS
        .iter()
        .find(|(candidate, _)| *candidate == id)
        .map_or("Texto", |(_, label)| label)
}

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Syntect grammar for an id. Languages missing from syntect's default set
/// borrow the closest grammar available.
fn syntax_for(id: &str) -> Option<&'static SyntaxReference> {
    let name = match id {
        "bash" | "docker" | "powershell" => "Bourne Again Shell (bash)",
        "c" => "C",
        "c++" => "C++",
        "c#" => "C#",
        "css" | "less" | "scss" => "CSS",
        "dart" | "java" | "kotlin" => "Java",
        "elixir" | "ruby" => "Ruby",
        "erlang" => "Erlang",
        "go" => "Go",
        "graphql" | "javascript" | "typescript" => "JavaScript",
        "groovy" => "Groovy",
        "haskell" => "Haskell",
        "html" => "HTML",
        "json" => "JSON",
        "latex" => "LaTeX",
        "lua" => "Lua",
        "makefile" => "Makefile",
        "markdown" => "Markdown",
        "matlab" => "MATLAB",
        "objective-c" => "Objective-C",
        "perl" => "Perl",
        "php" => "PHP",
        "python" | "toml" => "Python",
        "r" => "R",
        "rust" | "swift" => "Rust",
        "scala" => "Scala",
        "sql" => "SQL",
        "xml" => "XML",
        "yaml" => "YAML",
        _ => return None,
    };
    syntaxes().find_syntax_by_name(name)
}

fn color(c: Color32) -> Option<Color> {
    Some(Color {
        r: c.r(),
        g: c.g(),
        b: c.b(),
        a: 0xFF,
    })
}

fn build_theme(p: &Palette) -> Theme {
    let rules = [
        ("comment, punctuation.definition.comment", p.faint),
        ("string, punctuation.definition.string", p.code_string),
        (
            "constant.numeric, constant.language, constant.character, constant.other",
            p.code_number,
        ),
        ("keyword, storage, keyword.operator.word", p.accent),
        (
            "entity.name.function, support.function, variable.function",
            p.code_function,
        ),
        (
            "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, \
             support.type, support.class, storage.type.primitive",
            p.code_type,
        ),
        ("entity.name.tag", p.accent),
        ("entity.other.attribute-name", p.code_function),
        ("punctuation, keyword.operator", p.muted),
    ];
    Theme {
        settings: ThemeSettings {
            foreground: color(p.text),
            ..Default::default()
        },
        scopes: rules
            .iter()
            .filter_map(|(scope, fg)| {
                Some(ThemeItem {
                    scope: ScopeSelectors::from_str(scope).ok()?,
                    style: StyleModifier {
                        foreground: color(*fg),
                        ..Default::default()
                    },
                })
            })
            .collect(),
        ..Default::default()
    }
}

fn theme(dark: bool) -> &'static Theme {
    static THEMES: OnceLock<[Theme; 2]> = OnceLock::new();
    let themes = THEMES.get_or_init(|| {
        [
            build_theme(&palette(Mode::Light)),
            build_theme(&palette(Mode::Dark)),
        ]
    });
    &themes[usize::from(dark)]
}

#[derive(Default)]
struct Cache {
    entries: HashMap<u64, Spans>,
    order: VecDeque<u64>,
}

const CACHE_LIMIT: usize = 64;

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

fn uncolored(text: &str, p: &Palette) -> Vec<(Range<usize>, Color32)> {
    if text.is_empty() {
        Vec::new()
    } else {
        vec![(0..text.len(), p.text)]
    }
}

fn tokenize(text: &str, id: &str, p: &Palette) -> Vec<(Range<usize>, Color32)> {
    let Some(syntax) = syntax_for(id) else {
        return uncolored(text, p);
    };
    let mut lines = HighlightLines::new(syntax, theme(p.dark));
    let mut spans: Vec<(Range<usize>, Color32)> = Vec::new();
    let mut offset = 0;
    for line in LinesWithEndings::from(text) {
        let Ok(tokens) = lines.highlight_line(line, syntaxes()) else {
            return uncolored(text, p);
        };
        for (style, piece) in tokens {
            if piece.is_empty() {
                continue;
            }
            let fg = style.foreground;
            let color = Color32::from_rgb(fg.r, fg.g, fg.b);
            let end = offset + piece.len();
            match spans.last_mut() {
                Some((range, last)) if *last == color && range.end == offset => range.end = end,
                _ => spans.push((offset..end, color)),
            }
            offset = end;
        }
    }
    if offset != text.len() {
        return uncolored(text, p);
    }
    spans
}

/// Byte ranges of `text` with their colors, covering the whole text in order.
/// Results are cached per (text, language, mode).
pub fn highlight(text: &str, lang: &str, p: &Palette) -> Spans {
    let id = normalize(lang);
    if id == "plain" {
        return Arc::new(uncolored(text, p));
    }
    let mut hasher = DefaultHasher::new();
    (text, id, p.dark).hash(&mut hasher);
    let key = hasher.finish();

    if let Ok(cache) = cache().lock() {
        if let Some(spans) = cache.entries.get(&key) {
            return spans.clone();
        }
    }
    let spans = Arc::new(tokenize(text, id, p));
    if let Ok(mut cache) = cache().lock() {
        if cache.entries.insert(key, spans.clone()).is_none() {
            cache.order.push_back(key);
        }
        while cache.order.len() > CACHE_LIMIT {
            if let Some(old) = cache.order.pop_front() {
                cache.entries.remove(&old);
            }
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_covers(text: &str, spans: &[(Range<usize>, Color32)]) {
        let mut offset = 0;
        for (range, _) in spans {
            assert_eq!(range.start, offset);
            assert!(range.end > range.start);
            offset = range.end;
        }
        assert_eq!(offset, text.len());
    }

    #[test]
    fn normalize_resolves_aliases() {
        let cases = [
            ("js", "javascript"),
            ("ts", "typescript"),
            ("py", "python"),
            ("rs", "rust"),
            ("sh", "bash"),
            ("zsh", "bash"),
            ("yml", "yaml"),
            ("cpp", "c++"),
            ("c#", "c#"),
            ("cs", "c#"),
            ("objc", "objective-c"),
            ("md", "markdown"),
            ("ps1", "powershell"),
            ("rb", "ruby"),
            ("scss", "scss"),
            ("sass", "scss"),
            ("dockerfile", "docker"),
            ("JavaScript", "javascript"),
            ("", "plain"),
            ("desconhecida", "plain"),
        ];
        for (input, expected) in cases {
            assert_eq!(normalize(input), expected, "{input}");
        }
    }

    #[test]
    fn canonical_and_label() {
        assert_eq!(canonical("JavaScript"), "javascript");
        assert_eq!(canonical("mermaid"), "mermaid");
        assert_eq!(canonical("plaintext"), "");
        assert_eq!(label("c++"), "C++");
        assert_eq!(label("objc"), "Objective-C");
        assert_eq!(label(""), "Texto");
    }

    #[test]
    fn every_language_has_a_grammar() {
        assert!(LANGS.len() >= 40);
        let mut ids: Vec<&str> = LANGS.iter().map(|(id, _)| *id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), LANGS.len());
        for (id, _) in LANGS {
            assert_eq!(normalize(id), *id);
            if *id != "plain" {
                assert!(syntax_for(id).is_some(), "{id}");
            }
        }
    }

    #[test]
    fn highlight_colors_rust() {
        let p = palette(Mode::Dark);
        let text = "fn main() { let x = 1; }\n";
        let spans = highlight(text, "rust", &p);
        assert_covers(text, &spans);
        let mut colors: Vec<Color32> = spans.iter().map(|(_, color)| *color).collect();
        colors.sort_unstable_by_key(|color| color.to_array());
        colors.dedup();
        assert!(colors.len() >= 2);
    }

    #[test]
    fn unknown_language_is_plain() {
        let p = palette(Mode::Light);
        let spans = highlight("a = 1", "mermaid", &p);
        assert_eq!(spans.as_slice(), &[(0..5, p.text)]);
    }
}
