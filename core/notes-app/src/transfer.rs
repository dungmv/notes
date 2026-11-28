//! Import/Export Desktop 1.0. Ma trận giữ trường theo định dạng (roadmap M4):
//! - **Markdown**: nội dung, heading, checklist, bảng pipe, bold/italic/
//!   strikethrough, ảnh (thư mục `attachments/`), liên kết ghi chú
//!   `notes://note/<id>`, tag inline. Underline và highlight không có cú pháp.
//! - **HTML**: tương đương Markdown, thêm underline.
//! - **PDF**: nội dung văn bản + heading/checklist; ảnh và bảng chuyển thành
//!   mô tả văn bản, chưa nhúng ảnh (`TD-018`).
//! Ghi chú khóa không nằm trong export/import.

use crate::editor::{Block, BlockKind, Editor, Mark, ParagraphStyle, RichText, Style};
use crate::offline::{ImportedAttachment, OfflineError, Store};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn title_of(note_title: &str) -> String {
    let sanitized: String = note_title
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            _ => ch,
        })
        .collect();
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        "Ghi chú".to_owned()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn html_rich(rich: &RichText) -> String {
    // Mark dùng offset UTF-16; chuyển sang byte trước khi cắt chuỗi.
    let to_byte = |offset: u32| {
        crate::editor::utf16_to_byte(&rich.text, offset).unwrap_or(rich.text.len())
    };
    let mut boundaries: Vec<(usize, Style, bool)> = Vec::new();
    for mark in &rich.marks {
        boundaries.push((to_byte(mark.start), mark.style, true));
        boundaries.push((to_byte(mark.end), mark.style, false));
    }
    boundaries.sort_by_key(|(position, _, _)| *position);
    let mut out = String::new();
    let mut cursor = 0usize;
    let mut open: Vec<&'static str> = Vec::new();
    for (position, style, opening) in boundaries {
        if position > cursor {
            out.push_str(&escape_html(&rich.text[cursor..position]));
            cursor = position;
        }
        let (open_tag, close_tag) = match style {
            Style::Bold => ("<b>", "</b>"),
            Style::Italic => ("<i>", "</i>"),
            Style::Underline => ("<u>", "</u>"),
            Style::Strikethrough => ("<s>", "</s>"),
        };
        if opening {
            out.push_str(open_tag);
            open.push(close_tag);
        } else if let Some(last) = open.pop() {
            out.push_str(last);
        }
    }
    out.push_str(&escape_html(&rich.text[cursor..]));
    while let Some(last) = open.pop() {
        out.push_str(last);
    }
    out
}

fn markdown_rich(rich: &RichText) -> String {
    let mut out = rich.text.clone();
    let mut marks = rich.marks.clone();
    marks.sort_by_key(|mark| std::cmp::Reverse(mark.start));
    for mark in marks {
        let Ok(start) = crate::editor::utf16_to_byte(&rich.text, mark.start) else {
            continue;
        };
        let Ok(end) = crate::editor::utf16_to_byte(&rich.text, mark.end) else {
            continue;
        };
        if start >= end || end > out.len() {
            continue;
        }
        let wrap = match mark.style {
            Style::Bold => ("**", "**"),
            Style::Italic => ("*", "*"),
            Style::Strikethrough => ("~~", "~~"),
            // Underline không có cú pháp Markdown chuẩn.
            Style::Underline => ("", ""),
        };
        out = format!(
            "{}{}{}{}{}",
            &out[..start],
            wrap.0,
            &out[start..end],
            wrap.1,
            &out[end..]
        );
    }
    out
}

impl Store {
    fn note_blocks(&self, id: &str) -> Result<(String, Vec<Block>), OfflineError> {
        let (state, locked): (Vec<u8>, i64) = self.db().query_row(
            "SELECT crdt_state, is_locked FROM notes WHERE id=?1 AND deleted_at IS NULL",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if locked != 0 {
            return Err(OfflineError::NoteLocked);
        }
        let editor = Editor::from_state(&state)?;
        let snapshot = editor.snapshot();
        Ok((self.note(id)?.title().to_owned(), snapshot.blocks.clone()))
    }

    fn export_image_bytes(&self, attachment_id: &str, note_id: &str) -> Result<Vec<u8>, OfflineError> {
        let hash: String = self.db().query_row(
            "SELECT content_hash FROM attachments WHERE id=?1 AND note_id=?2",
            [attachment_id, note_id],
            |row| row.get(0),
        )?;
        self.read_blob_bytes(&hash)
    }

    /// Export một ghi chú ra Markdown kèm thư mục `attachments/`.
    pub fn export_markdown(&self, id: &str, out_dir: &Path) -> Result<PathBuf, OfflineError> {
        let (title, blocks) = self.note_blocks(id)?;
        let attachments = out_dir.join("attachments");
        let mut md = format!("# {}\n\n", title);
        for block in &blocks {
            match &block.kind {
                BlockKind::Paragraph { content } | BlockKind::Styled { content, .. } => {
                    md.push_str(&markdown_rich(content));
                    md.push_str("\n\n");
                }
                BlockKind::Checklist { content, depth, checked } => {
                    md.push_str(&"  ".repeat(*depth as usize));
                    md.push_str(if *checked { "- [x] " } else { "- [ ] " });
                    md.push_str(&markdown_rich(content));
                    md.push('\n');
                }
                BlockKind::Table { rows } => {
                    for (index, row) in rows.iter().enumerate() {
                        md.push_str("| ");
                        md.push_str(
                            &row.iter()
                                .map(|cell| markdown_rich(cell))
                                .collect::<Vec<_>>()
                                .join(" | "),
                        );
                        md.push_str(" |\n");
                        if index == 0 {
                            md.push_str(&format!(
                                "|{}|\n",
                                vec![" --- "; row.len()].join(" | ")
                            ));
                        }
                    }
                    md.push('\n');
                }
                BlockKind::Image { attachment_id } => {
                    let bytes = self.export_image_bytes(attachment_id, id)?;
                    let name = format!("{attachment_id}.png");
                    fs::create_dir_all(&attachments)?;
                    fs::write(attachments.join(&name), &bytes)?;
                    md.push_str(&format!("![{name}](attachments/{name})\n\n"));
                }
                BlockKind::NoteLink { note_id, label } => {
                    let text = label.clone().unwrap_or_else(|| {
                        self.note(note_id).map(|note| note.title().to_owned()).unwrap_or_default()
                    });
                    md.push_str(&format!("[{text}](notes://note/{note_id})\n\n"));
                }
            }
        }
        fs::create_dir_all(out_dir)?;
        let path = out_dir.join(format!("{}.md", title_of(&title)));
        fs::write(&path, md)?;
        Ok(path)
    }

    /// Export một ghi chú ra HTML (ảnh đặt cạnh file, src tương đối).
    pub fn export_html(&self, id: &str, out_dir: &Path) -> Result<PathBuf, OfflineError> {
        let (title, blocks) = self.note_blocks(id)?;
        let attachments = out_dir.join("attachments");
        let mut html = format!(
            "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>{}</title></head><body>\n",
            escape_html(&title)
        );
        let mut list_open = false;
        for block in &blocks {
            match &block.kind {
                BlockKind::Paragraph { content } => {
                    html.push_str(&format!("<p>{}</p>\n", html_rich(content)));
                }
                BlockKind::Styled { content, style } => {
                    let tag = match style {
                        ParagraphStyle::Title => "h1",
                        ParagraphStyle::Heading => "h2",
                        ParagraphStyle::Subheading => "h3",
                        ParagraphStyle::Quote => "blockquote",
                        ParagraphStyle::Monospaced => "pre",
                        ParagraphStyle::Bullet | ParagraphStyle::Dashed | ParagraphStyle::Numbered => {
                            if !list_open {
                                html.push_str("<ul>\n");
                                list_open = true;
                            }
                            "li"
                        }
                    };
                    html.push_str(&format!("<{tag}>{}</{tag}>\n", html_rich(content)));
                }
                BlockKind::Checklist { content, depth, checked } => {
                    if !list_open {
                        html.push_str("<ul>\n");
                        list_open = true;
                    }
                    let state = if *checked { " checked" } else { "" };
                    html.push_str(&format!(
                        "<li data-depth=\"{depth}\"><input type=\"checkbox\" disabled{state}/> {}</li>\n",
                        html_rich(content)
                    ));
                }
                BlockKind::Table { rows } => {
                    html.push_str("<table>\n");
                    for (index, row) in rows.iter().enumerate() {
                        html.push_str("<tr>");
                        for cell in row {
                            let tag = if index == 0 { "th" } else { "td" };
                            html.push_str(&format!("<{tag}>{}</{tag}>", html_rich(cell)));
                        }
                        html.push_str("</tr>\n");
                    }
                    html.push_str("</table>\n");
                }
                BlockKind::NoteLink { note_id, label } => {
                    let text = label.clone().unwrap_or_else(|| {
                        self.note(note_id).map(|note| note.title().to_owned()).unwrap_or_default()
                    });
                    html.push_str(&format!(
                        "<p><a href=\"notes://note/{note_id}\">{}</a></p>\n",
                        escape_html(&text)
                    ));
                }
                BlockKind::Image { attachment_id } => {
                    let bytes = self.export_image_bytes(attachment_id, id)?;
                    let name = format!("{attachment_id}.png");
                    fs::create_dir_all(&attachments)?;
                    fs::write(attachments.join(&name), &bytes)?;
                    html.push_str(&format!("<img src=\"attachments/{name}\" alt=\"{name}\"/>\n"));
                }
            }
        }
        if list_open {
            html.push_str("</ul>\n");
        }
        html.push_str("</body></html>\n");
        fs::create_dir_all(out_dir)?;
        let path = out_dir.join(format!("{}.html", title_of(&title)));
        fs::write(&path, html)?;
        Ok(path)
    }

    /// Export PDF: văn bản + heading/checklist với font nhúng hỗ trợ tiếng
    /// Việt; ảnh/bảng được mô tả dạng văn bản (`TD-018`).
    pub fn export_pdf(&self, id: &str, out_dir: &Path) -> Result<PathBuf, OfflineError> {
        use printpdf::*;
        let (title, blocks) = self.note_blocks(id)?;
        let font_path = ["arial.ttf", "segoeui.ttf", "calibri.ttf"]
            .iter()
            .map(|name| Path::new("C:\\Windows\\Fonts").join(name))
            .find(|path| path.is_file())
            .ok_or(OfflineError::Invalid)?;
        let font_bytes = fs::read(&font_path)?;
        let mut font_warnings = Vec::new();
        let parsed = ParsedFont::from_bytes(&font_bytes, 0, &mut font_warnings)
            .ok_or(OfflineError::Invalid)?;

        let mut doc = PdfDocument::new("Notes");
        let font_id = doc.add_font(&parsed);
        let font = PdfFontHandle::External(font_id);

        let mut pages: Vec<PdfPage> = Vec::new();
        let mut ops: Vec<Op> = Vec::new();
        let mut y = 285.0;
        let mut lines_on_page = 0u32;
        for block in &blocks {
            let lines: Vec<String> = match &block.kind {
                BlockKind::Paragraph { content } | BlockKind::Styled { content, .. } => {
                    wrap_line(&markdown_rich(content), 90)
                }
                BlockKind::Checklist { content, depth, checked } => wrap_line(
                    &format!(
                        "{}{} {}",
                        "    ".repeat(*depth as usize),
                        if *checked { "[x]" } else { "[ ]" },
                        markdown_rich(content)
                    ),
                    90,
                ),
                BlockKind::Table { rows } => rows
                    .iter()
                    .flat_map(|row| {
                        wrap_line(
                            &row.iter()
                                .map(|cell| markdown_rich(cell))
                                .collect::<Vec<_>>()
                                .join(" | "),
                            90,
                        )
                    })
                    .collect(),
                BlockKind::Image { .. } => {
                    vec!["[ảnh đính kèm — xem bản Markdown/HTML]".to_owned()]
                }
                BlockKind::NoteLink { note_id, label } => {
                    vec![format!(
                        "notes://note/{note_id} {}",
                        label.clone().unwrap_or_default()
                    )]
                }
            };
            for line in lines {
                if lines_on_page >= 38 {
                    pages.push(PdfPage::new(Mm(210.0), Mm(297.0), std::mem::take(&mut ops)));
                    lines_on_page = 0;
                    y = 285.0;
                }
                ops.push(Op::StartTextSection);
                ops.push(Op::SetTextCursor {
                    pos: Point::new(Mm(15.0), Mm(y)),
                });
                ops.push(Op::SetFont { font: font.clone(), size: Pt(12.0) });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text(line)],
                });
                ops.push(Op::EndTextSection);
                y -= 7.0;
                lines_on_page += 1;
            }
            y -= 3.0;
        }
        pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
        let mut warnings: Vec<PdfWarnMsg> = Vec::new();
        let bytes = doc
            .with_pages(pages)
            .save(&PdfSaveOptions::default(), &mut warnings);
        fs::create_dir_all(out_dir)?;
        let path = out_dir.join(format!("{}.pdf", title_of(&title)));
        fs::write(&path, bytes)?;
        Ok(path)
    }
}

fn wrap_line(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        if current.chars().count() + word.chars().count() + 1 > width && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    out.push(current);
    out
}

// ---- Import ----

/// Import một thư mục Markdown: mỗi `.md` là một ghi chú, thư mục con thành
/// thư mục con, ảnh tham chiếu tương đối vào blob store.
pub fn import_markdown_folder(
    store: &mut Store,
    dir: &Path,
    parent: Option<&str>,
) -> Result<Vec<String>, OfflineError> {
    let mut imported = Vec::new();
    import_markdown_dir(store, dir, parent.unwrap_or("inbox"), &mut imported)?;
    Ok(imported)
}

fn import_markdown_dir(
    store: &mut Store,
    dir: &Path,
    folder: &str,
    imported: &mut Vec<String>,
) -> Result<(), OfflineError> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.filter_map(|entry| entry.ok()).collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            let child = store.create_folder(&name, Some(folder))?;
            import_markdown_dir(store, &path, &child, imported)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            let content = fs::read_to_string(&path)?;
            let (_title, blocks, attachments) =
                parse_markdown_file(&content, path.parent().unwrap_or(Path::new(".")))?;
            let id = store.create_note_with_blocks(folder, blocks, attachments)?;
            imported.push(id);
        }
    }
    Ok(())
}

type ParsedFile = (Option<String>, Vec<Block>, Vec<ImportedAttachment>);

fn parse_markdown_file(content: &str, source_dir: &Path) -> Result<ParsedFile, OfflineError> {
    let mut title = None;
    let mut body = content;
    if let Some(rest) = body.strip_prefix("# ") {
        let (first, tail) = match rest.find('\n') {
            Some(index) => (&rest[..index], &rest[index + 1..]),
            None => (rest, ""),
        };
        title = Some(first.trim().to_owned());
        body = tail;
    }
    let mut blocks: Vec<Block> = Vec::new();
    if let Some(title) = title.as_ref() {
        blocks.push(Block {
            id: new_block_id(),
            kind: BlockKind::Styled {
                content: RichText { text: title.clone(), marks: Vec::new() },
                style: ParagraphStyle::Title,
            },
        });
    }
    let mut attachments: Vec<ImportedAttachment> = Vec::new();
    let mut lines = body.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "---" {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let depth = (indent / 2).min(6) as u8;
        // Ảnh và liên kết ghi chú phải khớp trước khi rơi về paragraph.
        if let Some(attachment) = parse_image(trimmed, source_dir, &mut attachments)? {
            blocks.push(Block {
                id: new_block_id(),
                kind: BlockKind::Image { attachment_id: attachment },
            });
            continue;
        }
        if let Some(link) = parse_note_link(trimmed) {
            blocks.push(link);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            blocks.push(Block {
                id: new_block_id(),
                kind: BlockKind::Checklist {
                    content: parse_inline(rest),
                    depth,
                    checked: false,
                },
            });
        } else if let Some(rest) = trimmed.strip_prefix("- [x] ") {
            blocks.push(Block {
                id: new_block_id(),
                kind: BlockKind::Checklist {
                    content: parse_inline(rest),
                    depth,
                    checked: true,
                },
            });
        } else if trimmed.starts_with('|') {
            let mut rows = vec![pipe_row(trimmed)];
            while let Some(next) = lines.peek() {
                if next.trim().starts_with('|') {
                    rows.push(pipe_row(lines.next().unwrap().trim()));
                } else {
                    break;
                }
            }
            rows.retain(|row| {
                !row.iter().all(|cell| {
                    cell.text.chars().all(|ch| ch == '-' || ch == ' ' || ch == ':')
                })
            });
            if !rows.is_empty() {
                blocks.push(Block { id: new_block_id(), kind: BlockKind::Table { rows } });
            }
        } else {
            let (content, style) = parse_line(trimmed);
            blocks.push(Block {
                id: new_block_id(),
                kind: match style {
                    Some(style) => BlockKind::Styled { content, style },
                    None => BlockKind::Paragraph { content },
                },
            });
        }
    }
    if blocks.is_empty() {
        blocks.push(Block {
            id: new_block_id(),
            kind: BlockKind::Paragraph { content: RichText::default() },
        });
    }
    Ok((title, blocks, attachments))
}

fn new_block_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn parse_line(line: &str) -> (RichText, Option<ParagraphStyle>) {
    if let Some(rest) = line.strip_prefix("# ") {
        return (parse_inline(rest), Some(ParagraphStyle::Title));
    }
    if let Some(rest) = line.strip_prefix("## ") {
        return (parse_inline(rest), Some(ParagraphStyle::Heading));
    }
    if let Some(rest) = line.strip_prefix("### ") {
        return (parse_inline(rest), Some(ParagraphStyle::Subheading));
    }
    if let Some(rest) = line.strip_prefix("> ") {
        return (parse_inline(rest), Some(ParagraphStyle::Quote));
    }
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return (parse_inline(rest), Some(ParagraphStyle::Bullet));
    }
    if let Some(rest) = trimmed.strip_prefix("+ ") {
        return (parse_inline(rest), Some(ParagraphStyle::Dashed));
    }
    if let Some(rest) = trimmed.strip_prefix("* ") {
        return (parse_inline(rest), Some(ParagraphStyle::Bullet));
    }
    if let Some(rest) = numbered_item(trimmed) {
        return (parse_inline(rest), Some(ParagraphStyle::Numbered));
    }
    (parse_inline(line), None)
}

fn numbered_item(line: &str) -> Option<&str> {
    let index = line.find(|ch: char| !ch.is_ascii_digit())?;
    let rest = line.get(index..)?.strip_prefix(". ")?;
    Some(rest)
}

fn parse_inline(text: &str) -> RichText {
    // Hỗ trợ **bold**, *italic*, ~~strikethrough~~; đánh dấu từ trái sang phải.
    let mut rich = RichText::default();
    let mut rest = text;
    let mut position = 0u32;
    loop {
        let next = ["**", "~~", "*"].iter().filter_map(|token| rest.find(token)).min();
        let Some(index) = next else { break };
        let (before, tail) = rest.split_at(index);
        rich.text.push_str(before);
        position += before.chars().count() as u32;
        let token_len = if tail.starts_with("**") {
            2
        } else if tail.starts_with("~~") {
            2
        } else {
            1
        };
        let token = &tail[..token_len];
        let style = match token {
            "**" => Style::Bold,
            "~~" => Style::Strikethrough,
            _ => Style::Italic,
        };
        rest = &tail[token_len..];
        let Some(close) = rest.find(token) else {
            // Token mở không đóng: giữ nguyên phần đã quét.
            rich.text.push_str(token);
            position += token_len as u32;
            continue;
        };
        let (inner, after) = rest.split_at(close);
        let start = position;
        rich.text.push_str(inner);
        position += inner.chars().count() as u32;
        if start < position {
            rich.marks.push(Mark { start, end: position, style });
        }
        rest = &after[token_len..];
    }
    rich.text.push_str(rest);
    rich
}

fn pipe_row(line: &str) -> Vec<RichText> {
    line.trim_matches('|')
        .split('|')
        .map(|cell| RichText {
            text: cell.trim().to_owned(),
            marks: Vec::new(),
        })
        .collect()
}

fn parse_note_link(line: &str) -> Option<Block> {
    if !line.starts_with('[') {
        return None;
    }
    const MARK: &str = "](notes://note/";
    let close = line.find(MARK)?;
    let label = &line[1..close];
    let note_id = line[close + MARK.len()..].strip_suffix(')')?;
    Some(Block {
        id: new_block_id(),
        kind: BlockKind::NoteLink {
            note_id: note_id.to_owned(),
            label: (!label.is_empty()).then(|| label.to_owned()),
        },
    })
}

fn parse_image(
    line: &str,
    source_dir: &Path,
    attachments: &mut Vec<ImportedAttachment>,
) -> Result<Option<String>, OfflineError> {
    let start = match line.find("![") {
        Some(index) => index,
        None => return Ok(None),
    };
    let Some(rest) = line.get(start + 2..) else {
        return Ok(None);
    };
    let Some(close) = rest.find("](") else {
        return Ok(None);
    };
    let target = rest[close + 2..].trim_end_matches(')');
    let path = source_dir.join(target.trim());
    let bytes = fs::read(&path)?;
    if bytes.len() > 32 * 1024 * 1024 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(None);
    }
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let id = new_block_id();
    attachments.push(ImportedAttachment {
        id: id.clone(),
        content_hash: hash,
        bytes,
        file_name: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "image.png".to_owned()),
    });
    Ok(Some(id))
}

/// Import Apple Notes qua bản export HTML: mỗi file là một ghi chú, tiêu đề
/// từ `<title>`/`<h1>`, nội dung là các dòng văn bản thuần.
pub fn import_apple_notes_html(
    store: &mut Store,
    dir: &Path,
    parent: Option<&str>,
) -> Result<Vec<String>, OfflineError> {
    let folder = parent.unwrap_or("inbox");
    let mut imported = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(dir)?.filter_map(|entry| entry.ok()).collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("html") {
            continue;
        }
        let html = fs::read_to_string(&path)?;
        let text = apple_html_text(&html);
        let title = apple_html_title(&html).or_else(|| {
            text.lines().next().map(|line| line.chars().take(160).collect())
        });
        let (title_blocks, body_text) = match title {
            Some(first) => (
                vec![Block {
                    id: new_block_id(),
                    kind: BlockKind::Styled {
                        content: RichText { text: first.clone(), marks: Vec::new() },
                        style: ParagraphStyle::Title,
                    },
                }],
                text.strip_prefix(&first).unwrap_or(&text).trim_start_matches('\n'),
            ),
            None => (Vec::new(), text.as_str()),
        };
        let mut blocks = title_blocks;
        for line in body_text.lines().filter(|line| !line.trim().is_empty()) {
            blocks.push(Block {
                id: new_block_id(),
                kind: BlockKind::Paragraph {
                    content: RichText { text: line.to_owned(), marks: Vec::new() },
                },
            });
        }
        if blocks.is_empty() {
            blocks.push(Block {
                id: new_block_id(),
                kind: BlockKind::Paragraph { content: RichText::default() },
            });
        }
        let id = store.create_note_with_blocks(folder, blocks, Vec::new())?;
        imported.push(id);
    }
    Ok(imported)
}

fn apple_html_text(html: &str) -> String {
    let mut text = String::new();
    let mut inside = false;
    let mut buffer = String::new();
    for ch in html.chars() {
        match ch {
            '<' => {
                inside = true;
                if !buffer.trim().is_empty() {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(buffer.trim());
                }
                buffer.clear();
            }
            '>' => {
                inside = false;
                buffer.clear();
            }
            _ if !inside => buffer.push(ch),
            _ => {}
        }
    }
    if !buffer.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(buffer.trim());
    }
    text
}

fn apple_html_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    for tag in ["<h1>", "<title>"] {
        if let Some(start) = lower.find(tag) {
            let rest = &html[start + tag.len()..];
            if let Some(end) = rest.find("</") {
                let title = rest[..end].trim();
                if !title.is_empty() {
                    return Some(title.to_owned());
                }
            }
        }
    }
    None
}
