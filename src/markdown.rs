//! Markdown ⇄ blocks, plus the inline formatter used while editing.
//!
//! The editor is markdown-native: block text keeps its inline markers
//! (`**bold**`, `` `code` ``…) and the editor renders them as real inline
//! styles, hiding the markers unless the caret is inside them.

use std::ops::Range;

use crate::model::{Block, BlockKind};

// ---------------------------------------------------------------------------
// Character / byte helpers
// ---------------------------------------------------------------------------

pub fn char_to_byte(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

#[allow(dead_code)]
pub fn byte_to_char(text: &str, byte_index: usize) -> usize {
    text[..byte_index.min(text.len())].chars().count()
}

// ---------------------------------------------------------------------------
// Inline parsing
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: bool,
}

impl InlineStyle {
    fn and(self, other: Self) -> Self {
        Self {
            bold: self.bold || other.bold,
            italic: self.italic || other.italic,
            strike: self.strike || other.strike,
            code: self.code || other.code,
            link: self.link || other.link,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlinePiece {
    pub range: Range<usize>,
    pub style: InlineStyle,
    /// Marker characters (e.g. the `**`) are rendered only when the caret is near them.
    pub marker: bool,
    pub group: usize,
    pub group_range: Range<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct InlineLayout {
    pub pieces: Vec<InlinePiece>,
    /// Clickable link ranges (content only) with their target.
    pub links: Vec<(Range<usize>, String)>,
    /// Ranges of inline emphasis (content only), used by the formatting toolbar.
    pub styles: Vec<(Range<usize>, InlineStyle)>,
}

pub fn parse_inline(text: &str) -> InlineLayout {
    let mut layout = InlineLayout::default();
    let mut group = 0;
    parse_segment(
        text,
        0,
        text.len(),
        InlineStyle::default(),
        &mut group,
        &mut layout,
    );
    layout
}

fn parse_segment(
    text: &str,
    start: usize,
    end: usize,
    style: InlineStyle,
    group: &mut usize,
    out: &mut InlineLayout,
) {
    if start >= end {
        return;
    }
    let mut i = start;
    let mut run = start;

    while i < end {
        let rest = &text[i..end];

        // Escaped marker characters: hide the backslash, keep the character.
        if rest.starts_with('\\') {
            let mut chars = rest.chars();
            chars.next();
            if let Some(next) = chars.next() {
                if is_marker_char(next) {
                    push_plain(out, run, i, style);
                    push_piece(out, i..i + 1, style, true, usize::MAX, 0..0);
                    i += 1;
                    run = i;
                    continue;
                }
            }
            i += 1;
            continue;
        }

        // `inline code`
        if rest.starts_with('`') {
            if let Some(rel) = text[i + 1..end].find('`') {
                let close = i + 1 + rel;
                if close > i + 1 {
                    push_plain(out, run, i, style);
                    *group += 1;
                    let g = *group;
                    let gr = i..close + 1;
                    push_piece(out, i..i + 1, style, true, g, gr.clone());
                    let code_style = InlineStyle {
                        code: true,
                        ..style
                    };
                    push_piece(out, i + 1..close, code_style, false, 0, 0..0);
                    out.styles.push((i + 1..close, code_style));
                    push_piece(out, close..close + 1, style, true, g, gr);
                    i = close + 1;
                    run = i;
                    continue;
                }
            }
        }

        // [text](url)
        if rest.starts_with('[') {
            if let Some((content_end, url_end, url)) = parse_link(text, i, end) {
                push_plain(out, run, i, style);
                *group += 1;
                let g = *group;
                let gr = i..url_end;
                push_piece(out, i..i + 1, style, true, g, gr.clone());
                let link_style = InlineStyle {
                    link: true,
                    ..style
                };
                parse_segment(text, i + 1, content_end, link_style, group, out);
                out.styles.push((i + 1..content_end, link_style));
                push_piece(out, content_end..url_end, style, true, g, gr);
                out.links.push((i + 1..content_end, url));
                i = url_end;
                run = i;
                continue;
            }
        }

        // Emphasis markers.
        let mut matched = false;
        for (marker, delta, word_boundary) in EMPHASIS {
            if !rest.starts_with(marker) {
                continue;
            }
            if word_boundary && !open_boundary_ok(text, i, marker.len()) {
                continue;
            }
            if let Some(close) = find_close(text, i + marker.len(), end, marker, word_boundary) {
                if close == i + marker.len() {
                    continue;
                }
                push_plain(out, run, i, style);
                *group += 1;
                let g = *group;
                let gr = i..close + marker.len();
                push_piece(out, i..i + marker.len(), style, true, g, gr.clone());
                let inner = style.and(delta);
                parse_segment(text, i + marker.len(), close, inner, group, out);
                out.styles.push((i + marker.len()..close, inner));
                push_piece(out, close..close + marker.len(), style, true, g, gr);
                i = close + marker.len();
                run = i;
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        let ch = text[i..end].chars().next().unwrap_or('\n');
        i += ch.len_utf8();
    }

    push_plain(out, run, end, style);
}

const EMPHASIS: [(&str, InlineStyle, bool); 5] = [
    (
        "**",
        InlineStyle {
            bold: true,
            italic: false,
            strike: false,
            code: false,
            link: false,
        },
        false,
    ),
    (
        "__",
        InlineStyle {
            bold: true,
            italic: false,
            strike: false,
            code: false,
            link: false,
        },
        true,
    ),
    (
        "~~",
        InlineStyle {
            bold: false,
            italic: false,
            strike: true,
            code: false,
            link: false,
        },
        false,
    ),
    (
        "*",
        InlineStyle {
            bold: false,
            italic: true,
            strike: false,
            code: false,
            link: false,
        },
        false,
    ),
    (
        "_",
        InlineStyle {
            bold: false,
            italic: true,
            strike: false,
            code: false,
            link: false,
        },
        true,
    ),
];

fn is_marker_char(c: char) -> bool {
    matches!(c, '*' | '_' | '`' | '~' | '[' | ']' | '\\')
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

fn open_boundary_ok(text: &str, open: usize, marker_len: usize) -> bool {
    let before = text[..open].chars().next_back();
    let after = text[open + marker_len..].chars().next();
    before.is_none_or(|c| !is_word(c)) && after.is_some_and(|c| !c.is_whitespace())
}

fn close_boundary_ok(text: &str, close: usize, marker_len: usize) -> bool {
    let before = text[..close].chars().next_back();
    let after = text[close + marker_len..].chars().next();
    before.is_some_and(|c| !c.is_whitespace()) && after.is_none_or(|c| !is_word(c))
}

fn find_close(
    text: &str,
    from: usize,
    end: usize,
    marker: &str,
    word_boundary: bool,
) -> Option<usize> {
    let marker_char = marker.chars().next()?;
    let marker_len = marker.len();
    let mut i = from;
    while i < end {
        if text[i..end].starts_with(marker) {
            // Prefer the rightmost alignment inside a run of marker characters,
            // so that `**a *b***` closes the italic before the bold.
            let mut run_end = i + marker_len;
            while run_end < end && text[run_end..end].starts_with(marker_char) {
                run_end += marker_char.len_utf8();
            }
            let candidate = run_end - marker_len;
            let escaped = candidate > 0 && text[..candidate].ends_with('\\');
            let boundary_ok = !word_boundary || close_boundary_ok(text, candidate, marker_len);
            if candidate > from && !escaped && candidate + marker_len <= end && boundary_ok {
                return Some(candidate);
            }
            i = run_end;
            continue;
        }
        let ch = text[i..end].chars().next()?;
        i += ch.len_utf8();
    }
    None
}

fn parse_link(text: &str, open: usize, end: usize) -> Option<(usize, usize, String)> {
    let mut i = open + 1;
    let close_text = loop {
        if i >= end {
            return None;
        }
        let ch = text[i..end].chars().next()?;
        if ch == '\\' {
            i += 1;
            if i < end {
                i += text[i..end].chars().next()?.len_utf8();
            }
            continue;
        }
        if ch == ']' {
            break i;
        }
        i += ch.len_utf8();
    };
    if !text[close_text + 1..end].starts_with('(') {
        return None;
    }
    let url_start = close_text + 2;
    let mut j = url_start;
    while j < end {
        let ch = text[j..end].chars().next()?;
        if ch == ')' {
            let url = text[url_start..j].trim().to_string();
            if url.is_empty() {
                return None;
            }
            return Some((close_text, j + 1, url));
        }
        if ch == '\n' {
            return None;
        }
        j += ch.len_utf8();
    }
    None
}

fn push_plain(out: &mut InlineLayout, start: usize, end: usize, style: InlineStyle) {
    if start < end {
        push_piece(out, start..end, style, false, 0, 0..0);
    }
}

fn push_piece(
    out: &mut InlineLayout,
    range: Range<usize>,
    style: InlineStyle,
    marker: bool,
    group: usize,
    group_range: Range<usize>,
) {
    if range.is_empty() {
        return;
    }
    if let Some(last) = out.pieces.last_mut() {
        if last.range.end == range.start
            && last.style == style
            && last.marker == marker
            && last.group == group
            && last.group_range == group_range
        {
            last.range.end = range.end;
            return;
        }
    }
    out.pieces.push(InlinePiece {
        range,
        style,
        marker,
        group,
        group_range,
    });
}

// ---------------------------------------------------------------------------
// Inline formatting commands (Ctrl+B and friends)
// ---------------------------------------------------------------------------

/// Wrap or unwrap `sel` (character indices) with `marker`.
/// Returns the new text and the new character selection.
pub fn toggle_wrap(text: &str, sel: (usize, usize), marker: &str) -> (String, (usize, usize)) {
    let chars = text.chars().count();
    let a = sel.0.min(chars);
    let b = sel.1.min(chars).max(a);
    let m = marker.chars().count();
    let bs = char_to_byte(text, a);
    let be = char_to_byte(text, b);

    if a == b {
        let mut out = String::with_capacity(text.len() + marker.len() * 2);
        out.push_str(&text[..bs]);
        out.push_str(marker);
        out.push_str(marker);
        out.push_str(&text[bs..]);
        return (out, (a + m, a + m));
    }

    // Already wrapped from the outside: **|text|**
    if bs >= marker.len()
        && be + marker.len() <= text.len()
        && text.is_char_boundary(bs - marker.len())
        && text[..bs].ends_with(marker)
        && text[be..].starts_with(marker)
    {
        let mut out = String::with_capacity(text.len());
        out.push_str(&text[..bs - marker.len()]);
        out.push_str(&text[bs..be]);
        out.push_str(&text[be + marker.len()..]);
        return (out, (a - m, b - m));
    }

    // The selection itself includes the markers: |**text**|
    if b - a >= 2 * m && text[bs..].starts_with(marker) && text[..be].ends_with(marker) {
        let mut out = String::with_capacity(text.len());
        out.push_str(&text[..bs]);
        out.push_str(&text[bs + marker.len()..be - marker.len()]);
        out.push_str(&text[be..]);
        return (out, (a, b - 2 * m));
    }

    let mut out = String::with_capacity(text.len() + marker.len() * 2);
    out.push_str(&text[..bs]);
    out.push_str(marker);
    out.push_str(&text[bs..be]);
    out.push_str(marker);
    out.push_str(&text[be..]);
    (out, (a + m, b + m))
}

pub fn wrap_link(text: &str, sel: (usize, usize), url: &str) -> (String, (usize, usize)) {
    let chars = text.chars().count();
    let a = sel.0.min(chars);
    let b = sel.1.min(chars).max(a);
    let bs = char_to_byte(text, a);
    let be = char_to_byte(text, b);
    let label = if a == b { "texto" } else { &text[bs..be] };

    let mut out = String::with_capacity(text.len() + label.len() + url.len() + 4);
    out.push_str(&text[..bs]);
    out.push('[');
    let label_start = a + 1;
    out.push_str(label);
    out.push_str("](");
    let url_start = label_start + label.chars().count() + 2;
    out.push_str(url);
    out.push(')');
    out.push_str(&text[be..]);

    (out, (url_start, url_start + url.chars().count()))
}

// ---------------------------------------------------------------------------
// Markdown → blocks
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockDraft {
    pub kind: BlockKind,
    pub text: String,
    pub checked: bool,
    pub indent: u8,
    pub lang: String,
    pub emoji: String,
}

impl BlockDraft {
    pub fn into_block(self, id: u64) -> Block {
        Block {
            id,
            kind: self.kind,
            text: self.text,
            checked: self.checked,
            indent: self.indent,
            lang: self.lang,
            emoji: self.emoji,
        }
    }
}

/// Heuristic: should a pasted string be turned into blocks?
pub fn looks_like_markdown(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    if !text.contains('\n') {
        return false;
    }
    if text.contains("\n\n") {
        return true;
    }
    text.lines()
        .any(|line| line_starts_a_block(line.trim_start()))
}

fn line_starts_a_block(line: &str) -> bool {
    let line = line.trim_start();
    if line.starts_with("# ")
        || line.starts_with("## ")
        || line.starts_with("### ")
        || line.starts_with("- ")
        || line.starts_with("* ")
        || line.starts_with("+ ")
        || line.starts_with("> ")
        || line.starts_with("```")
        || line.starts_with("~~~")
        || line == "---"
        || line == "***"
        || line == "___"
    {
        return true;
    }
    if line.starts_with("- [") || line.starts_with("* [") {
        return true;
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && (line[digits..].starts_with(". ") || line[digits..].starts_with(") "))
}

fn split_indent(line: &str) -> (u8, &str) {
    let mut spaces = 0usize;
    for ch in line.chars() {
        match ch {
            ' ' => spaces += 1,
            '\t' => spaces += 4,
            _ => break,
        }
    }
    let indent = (spaces / 2).min(6) as u8;
    let trimmed = line.trim_start_matches([' ', '\t']);
    (indent, trimmed)
}

fn parse_todo(content: &str) -> Option<(bool, String)> {
    let rest = content
        .strip_prefix("- ")
        .or_else(|| content.strip_prefix("* "))
        .or_else(|| content.strip_prefix("+ "))?;
    let rest = rest.strip_prefix('[')?;
    let mut chars = rest.chars();
    let mark = chars.next()?;
    let after = chars.as_str();
    match mark {
        ']' => Some((false, after.strip_prefix(' ').unwrap_or(after).to_string())),
        ' ' | 'x' | 'X' => {
            let after = after
                .strip_prefix("] ")
                .or_else(|| after.strip_prefix(']'))?;
            Some((mark != ' ', after.to_string()))
        }
        _ => None,
    }
}

fn parse_numbered(content: &str) -> Option<String> {
    let digits = content.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = &content[digits..];
    let rest = rest
        .strip_prefix(". ")
        .or_else(|| rest.strip_prefix(") "))?;
    Some(rest.to_string())
}

fn parse_block_line(line: &str) -> Option<(BlockKind, String)> {
    let (indent, content) = split_indent(line);
    let _ = indent;
    let content = content.trim_end();

    if let Some((checked, text)) = parse_todo(content) {
        return Some((
            BlockKind::Todo,
            if checked {
                format!("\u{0}{text}")
            } else {
                text
            },
        ));
    }
    if let Some(rest) = content.strip_prefix("- ") {
        return Some((BlockKind::Bullet, rest.to_string()));
    }
    if let Some(rest) = content.strip_prefix("* ") {
        return Some((BlockKind::Bullet, rest.to_string()));
    }
    if let Some(rest) = content.strip_prefix("+ ") {
        return Some((BlockKind::Bullet, rest.to_string()));
    }
    if let Some(rest) = parse_numbered(content) {
        return Some((BlockKind::Numbered, rest));
    }
    if content.starts_with('>') {
        let inner = content[1..].trim_start();
        if let Some(after) = inner.strip_prefix("[!") {
            if let Some(close) = after.find(']') {
                let text = after[close + 1..].trim_start().to_string();
                return Some((BlockKind::Callout, text));
            }
        }
        return Some((BlockKind::Quote, inner.to_string()));
    }
    let hashes = content.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && content[hashes..].starts_with(' ') {
        let level = hashes.min(3);
        let kind = match level {
            1 => BlockKind::Heading1,
            2 => BlockKind::Heading2,
            _ => BlockKind::Heading3,
        };
        return Some((kind, content[hashes..].trim_start().to_string()));
    }
    let divider = content.trim();
    if divider.len() >= 3
        && (divider.chars().all(|c| c == '-')
            || divider.chars().all(|c| c == '*')
            || divider.chars().all(|c| c == '_'))
    {
        return Some((BlockKind::Divider, String::new()));
    }
    None
}

/// Parse markdown into block drafts. When `merge_paragraphs` is true, plain
/// lines separated by single newlines are joined into one paragraph (the way
/// markdown renders them).
pub fn parse_blocks(markdown: &str, merge_paragraphs: bool) -> Vec<BlockDraft> {
    let normalized = markdown.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut out: Vec<BlockDraft> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let (indent, content) = split_indent(line);
        let content_trimmed = content.trim_end();

        // Fenced code.
        if content_trimmed.starts_with("```") || content_trimmed.starts_with("~~~") {
            let fence = content_trimmed.chars().next().unwrap_or('`');
            let fence_len = content_trimmed
                .chars()
                .take_while(|&c| c == fence)
                .count()
                .max(3);
            let lang = content_trimmed[fence_len..].trim().to_string();
            let mut code = String::new();
            i += 1;
            while i < lines.len() {
                let (_, raw) = split_indent(lines[i]);
                let trimmed = raw.trim_end();
                let is_fence_close = trimmed.len() >= fence_len
                    && trimmed.chars().all(|c| c == fence)
                    && !trimmed.is_empty();
                if is_fence_close {
                    i += 1;
                    break;
                }
                if !code.is_empty() {
                    code.push('\n');
                }
                code.push_str(lines[i]);
                i += 1;
            }
            out.push(BlockDraft {
                kind: BlockKind::Code,
                text: code,
                indent,
                lang,
                ..Default::default()
            });
            continue;
        }

        if content_trimmed.trim().is_empty() {
            i += 1;
            continue;
        }

        if let Some((mut kind, mut text)) = parse_block_line(line) {
            let mut checked = false;
            if kind == BlockKind::Todo && text.starts_with('\u{0}') {
                checked = true;
                text.remove(0);
            }

            // Continuation lines for quotes / callouts.
            if matches!(kind, BlockKind::Quote | BlockKind::Callout) {
                i += 1;
                while i < lines.len() {
                    let (next_indent, next) = split_indent(lines[i]);
                    let next_trimmed = next.trim_end();
                    if next_indent != indent || !next_trimmed.starts_with('>') {
                        break;
                    }
                    let inner = next_trimmed[1..].trim_start();
                    if inner.starts_with("[!") {
                        break;
                    }
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(inner);
                    i += 1;
                }
                out.push(BlockDraft {
                    kind,
                    text,
                    checked,
                    indent,
                    emoji: if kind == BlockKind::Callout {
                        "💡".to_owned()
                    } else {
                        String::new()
                    },
                    ..Default::default()
                });
                continue;
            }

            if kind == BlockKind::Paragraph {
                // Handled below (paragraph grouping); parse_block_line never
                // returns Paragraph, but keep the branch for safety.
                kind = BlockKind::Paragraph;
            }

            out.push(BlockDraft {
                kind,
                text,
                checked,
                indent,
                ..Default::default()
            });
            i += 1;
            continue;
        }

        // Plain paragraph line.
        let hard_break = line.ends_with("  ");
        let text = content_trimmed.trim().to_string();
        i += 1;

        if merge_paragraphs {
            if let Some(last) = out.last_mut() {
                if last.kind == BlockKind::Paragraph && last.indent == indent && !hard_break {
                    if !text.is_empty() {
                        if !last.text.is_empty() {
                            last.text.push(' ');
                        }
                        last.text.push_str(&text);
                    }
                    while i < lines.len() {
                        let (next_indent, next) = split_indent(lines[i]);
                        let next_trimmed = next.trim_end();
                        if next_trimmed.trim().is_empty()
                            || next_indent != indent
                            || parse_block_line(lines[i]).is_some()
                            || lines[i].starts_with("```")
                        {
                            break;
                        }
                        last.text.push(' ');
                        last.text.push_str(next_trimmed.trim());
                        i += 1;
                    }
                    continue;
                }
            }
        }

        out.push(BlockDraft {
            kind: BlockKind::Paragraph,
            text,
            indent,
            ..Default::default()
        });
    }

    out
}

// ---------------------------------------------------------------------------
// Blocks → markdown
// ---------------------------------------------------------------------------

pub fn to_markdown(blocks: &[Block]) -> String {
    let mut out = String::new();

    for (index, block) in blocks.iter().enumerate() {
        if index > 0 && (needs_blank_line(blocks[index - 1].kind) || needs_blank_line(block.kind)) {
            out.push('\n');
        }
        let indent = "  ".repeat(block.indent as usize);
        match block.kind {
            BlockKind::Paragraph => {
                out.push_str(&indent);
                out.push_str(&block.text);
                out.push('\n');
            }
            BlockKind::Heading1 => {
                out.push_str(&format!("# {}\n", block.text));
            }
            BlockKind::Heading2 => {
                out.push_str(&format!("## {}\n", block.text));
            }
            BlockKind::Heading3 => {
                out.push_str(&format!("### {}\n", block.text));
            }
            BlockKind::Bullet => {
                out.push_str(&format!("{indent}- {}\n", block.text));
            }
            BlockKind::Numbered => {
                out.push_str(&format!("{indent}1. {}\n", block.text));
            }
            BlockKind::Todo => {
                let mark = if block.checked { "x" } else { " " };
                out.push_str(&format!("{indent}- [{mark}] {}\n", block.text));
            }
            BlockKind::Quote => {
                for line in block.text.lines() {
                    out.push_str(&format!("{indent}> {line}\n"));
                }
            }
            BlockKind::Callout => {
                let mut first = true;
                for line in block.text.lines() {
                    if first {
                        out.push_str(&format!("{indent}> [!NOTE] {line}\n"));
                        first = false;
                    } else {
                        out.push_str(&format!("{indent}> {line}\n"));
                    }
                }
                if block.text.is_empty() {
                    out.push_str(&format!("{indent}> [!NOTE]\n"));
                }
            }
            BlockKind::Code => {
                let fence = if block
                    .text
                    .lines()
                    .any(|l| l.trim_start().starts_with("```"))
                {
                    "~~~"
                } else {
                    "```"
                };
                out.push_str(&format!("{indent}{fence}{}\n", block.lang));
                out.push_str(&block.text);
                out.push('\n');
                out.push_str(&format!("{indent}{fence}\n"));
            }
            BlockKind::Divider => {
                out.push_str(&format!("{indent}---\n"));
            }
        }
    }

    out.trim_end().to_string() + "\n"
}

fn needs_blank_line(kind: BlockKind) -> bool {
    !kind.is_list()
}

/// Markdown for a whole page, including its title.
pub fn page_markdown(title: &str, blocks: &[Block]) -> String {
    let body = to_markdown(blocks);
    let title = title.trim();
    if title.is_empty() {
        body
    } else {
        format!("# {title}\n\n{body}")
    }
}

// ---------------------------------------------------------------------------
// Block-kind shortcuts
// ---------------------------------------------------------------------------

/// Detects markdown shortcuts while typing ("# ", "- ", "```" …).
/// Returns the new kind and how many characters to strip from the start.
pub fn shortcut_for(text: &str) -> Option<(BlockKind, usize)> {
    let candidates: [(&str, BlockKind); 14] = [
        ("### ", BlockKind::Heading3),
        ("## ", BlockKind::Heading2),
        ("# ", BlockKind::Heading1),
        ("- [ ] ", BlockKind::Todo),
        ("- [x] ", BlockKind::Todo),
        ("[ ] ", BlockKind::Todo),
        ("[] ", BlockKind::Todo),
        ("- ", BlockKind::Bullet),
        ("* ", BlockKind::Bullet),
        ("+ ", BlockKind::Bullet),
        ("1. ", BlockKind::Numbered),
        ("> ", BlockKind::Quote),
        ("```", BlockKind::Code),
        ("---", BlockKind::Divider),
    ];
    for (prefix, kind) in candidates {
        // Shortcuts that end in a space only trigger once the space is typed;
        // "```" and "---" trigger as soon as they are complete.
        if text == prefix {
            return Some((kind, prefix.chars().count()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(id: u64, kind: BlockKind, text: &str) -> Block {
        Block::with_text(id, kind, text)
    }

    #[test]
    fn inline_bold_is_hidden_but_mapped() {
        let text = "um **texto** legal";
        let layout = parse_inline(text);
        assert_eq!(
            layout
                .pieces
                .iter()
                .map(|p| &text[p.range.clone()])
                .collect::<String>(),
            text
        );
        assert!(
            layout
                .pieces
                .iter()
                .any(|p| p.marker && &text[p.range.clone()] == "**")
        );
        assert!(
            layout
                .pieces
                .iter()
                .any(|p| p.style.bold && &text[p.range.clone()] == "texto")
        );
    }

    #[test]
    fn inline_nested_styles() {
        let text = "**negrito com *itálico***";
        let layout = parse_inline(text);
        assert!(
            layout
                .pieces
                .iter()
                .any(|p| p.style.bold && p.style.italic && &text[p.range.clone()] == "itálico")
        );
    }

    #[test]
    fn inline_code_has_priority() {
        let text = "`**não é negrito**`";
        let layout = parse_inline(text);
        assert!(
            layout
                .pieces
                .iter()
                .any(|p| p.style.code && &text[p.range.clone()] == "**não é negrito**")
        );
    }

    #[test]
    fn inline_link() {
        let text = "veja [o site](https://exemplo.com) agora";
        let layout = parse_inline(text);
        assert_eq!(layout.links.len(), 1);
        assert_eq!(layout.links[0].1, "https://exemplo.com");
        assert_eq!(&text[layout.links[0].0.clone()], "o site");
    }

    #[test]
    fn toggle_bold_wraps_and_unwraps() {
        let (text, sel) = toggle_wrap("palavra", (0, 7), "**");
        assert_eq!(text, "**palavra**");
        assert_eq!(sel, (2, 9));
        let (text, sel) = toggle_wrap(&text, sel, "**");
        assert_eq!(text, "palavra");
        assert_eq!(sel, (0, 7));
    }

    #[test]
    fn parses_common_markdown() {
        let md = "# Título\n\nUm parágrafo\ncom duas linhas.\n\n- item\n- [x] feito\n\n1. um\n2. dois\n\n> citação\n\n```rust\nfn main() {}\n```\n\n---\n";
        let drafts = parse_blocks(md, true);
        let kinds: Vec<BlockKind> = drafts.iter().map(|d| d.kind).collect();
        assert_eq!(
            kinds,
            vec![
                BlockKind::Heading1,
                BlockKind::Paragraph,
                BlockKind::Bullet,
                BlockKind::Todo,
                BlockKind::Numbered,
                BlockKind::Numbered,
                BlockKind::Quote,
                BlockKind::Code,
                BlockKind::Divider,
            ]
        );
        assert_eq!(drafts[1].text, "Um parágrafo com duas linhas.");
        assert!(drafts[3].checked);
        assert_eq!(drafts[7].lang, "rust");
    }

    #[test]
    fn markdown_roundtrip() {
        let blocks = vec![
            block(1, BlockKind::Heading2, "Título"),
            block(2, BlockKind::Paragraph, "Texto com **negrito**."),
            block(3, BlockKind::Bullet, "item"),
            block(4, BlockKind::Todo, "tarefa"),
            block(5, BlockKind::Code, "fn main() {\n    println!(\"oi\");\n}"),
            block(6, BlockKind::Divider, ""),
        ];
        let mut blocks = blocks;
        blocks[3].checked = true;
        let md = to_markdown(&blocks);
        let drafts = parse_blocks(&md, true);
        assert_eq!(drafts.len(), 6);
        assert_eq!(drafts[3].checked, true);
        assert_eq!(drafts[4].kind, BlockKind::Code);
        assert!(drafts[4].text.contains("println"));
    }

    #[test]
    fn shortcuts_need_their_trigger() {
        assert!(shortcut_for("-").is_none());
        assert!(shortcut_for("#").is_none());
        assert!(shortcut_for(">").is_none());
        assert_eq!(shortcut_for("- ").map(|s| s.0), Some(BlockKind::Bullet));
        assert_eq!(shortcut_for("# ").map(|s| s.0), Some(BlockKind::Heading1));
        assert_eq!(shortcut_for("> ").map(|s| s.0), Some(BlockKind::Quote));
        assert_eq!(shortcut_for("[ ] ").map(|s| s.0), Some(BlockKind::Todo));
        assert_eq!(shortcut_for("```").map(|s| s.0), Some(BlockKind::Code));
        assert_eq!(shortcut_for("---").map(|s| s.0), Some(BlockKind::Divider));
    }

    #[test]
    fn char_byte_roundtrip() {
        let text = "ação 🚀 fim";
        for (i, _) in text.char_indices() {
            let c = byte_to_char(text, i);
            assert_eq!(char_to_byte(text, c), i);
        }
    }
}
