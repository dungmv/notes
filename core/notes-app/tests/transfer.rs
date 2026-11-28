use notes_app::editor::{BlockKind, Cell, Operation, Style};
use notes_app::offline::{INBOX, Store};
use notes_app::transfer::{import_apple_notes_html, import_markdown_folder};
use std::path::Path;

const PNG: &[u8] = include_bytes!("../../../fixtures/m1/pixel.png");

fn batch(revision: u64, operations: Vec<Operation>) -> notes_app::editor::Batch {
    notes_app::editor::Batch { revision, operations, external: false }
}

#[test]
fn export_import_round_trip_keeps_fields() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(&temp.path().join("a")).unwrap();
    let id = store.create_note(INBOX).unwrap();
    let mut editor = store.open_note(&id).unwrap();
    let block = editor.snapshot().blocks[0].id.clone();
    store.apply(&mut editor, batch(0, vec![Operation::Replace {
        id: block.clone(), cell: None, start: 0, end: 0,
        text: "Ghi chú quan trọng #kiem-tra".into(),
    }])).unwrap();
    store.apply(&mut editor, batch(1, vec![Operation::Format {
        id: block.clone(), cell: None, start: 8, end: 18, style: Style::Bold,
    }])).unwrap();
    // Checklist block sau đoạn văn.
    // "Ghi chú quan trọng #kiem-tra" dài 28 đơn vị UTF-16: tách ở cuối dòng.
    store.apply(&mut editor, batch(2, vec![Operation::SplitParagraph {
        id: block.clone(), offset: 28, new_id: "cl-1".into() }])).unwrap();
    store.apply(&mut editor, batch(3, vec![Operation::ParagraphStyle {
        id: "cl-1".into(), style: None, checklist: true }])).unwrap();
    store.apply(&mut editor, batch(4, vec![Operation::Replace {
        id: "cl-1".into(), cell: None, start: 0, end: 0, text: "Việc cần làm".into() }])).unwrap();
    store.paste_image(&mut editor, 5, "cl-1".into(), 12, 12, PNG).unwrap();

    // Bảng 2x2 chèn sau ảnh.
    let table = serde_json::json!({
        "id": "tbl-1", "kind": "table",
        "rows": [[{"text":"Một","marks":[]},{"text":"Hai","marks":[]}]]
    });
    let tail_block = editor.snapshot().blocks.last().unwrap().id.clone();
    store.apply(&mut editor, batch(6, vec![Operation::InsertAt {
        id: tail_block, start: 0, end: 0, block: serde_json::from_value(table).unwrap(),
        tail_id: "tail-2".into() }])).unwrap();

    let out = temp.path().join("export");
    let md = store.export_markdown(&id, &out).unwrap();
    assert!(md.is_file());
    let text = std::fs::read_to_string(&md).unwrap();
    assert!(text.contains("# Ghi chú quan trọng #kiem-tra"), "{text}");
    assert!(text.contains("Ghi chú **quan trọng** #kiem-tra"), "{text}");
    assert!(text.contains("- [ ] Việc cần làm"), "{text}");
    assert!(text.contains("![") && text.contains("](attachments/"), "{text}");
    assert!(text.contains("| Một | Hai |"), "{text}");

    let html = store.export_html(&id, &out).unwrap();
    let html_text = std::fs::read_to_string(&html).unwrap();
    assert!(html_text.contains("<b>quan trọng</b>"), "{html_text}");
    assert!(html_text.contains("<table>"), "{html_text}");

    let pdf = store.export_pdf(&id, &out).unwrap();
    let pdf_bytes = std::fs::read(&pdf).unwrap();
    assert!(pdf_bytes.starts_with(b"%PDF"));

    // Import ngược vào store khác và đối chiếu nội dung giữ trường.
    let mut store2 = Store::open(&temp.path().join("b")).unwrap();
    let imported = import_markdown_folder(&mut store2, &out, None).unwrap();
    assert_eq!(imported.len(), 1);
    let note_id = &imported[0];
    let editor2 = store2.open_note(note_id).unwrap();
    let snapshot = editor2.snapshot();
    let kinds: Vec<&str> = snapshot.blocks.iter().map(|b| match &b.kind {
        BlockKind::Paragraph { .. } | BlockKind::Styled { .. } => "text",
        BlockKind::Checklist { .. } => "checklist",
        BlockKind::Table { .. } => "table",
        BlockKind::Image { .. } => "image",
        BlockKind::NoteLink { .. } => "link",
    }).collect();
    assert!(kinds.contains(&"checklist"), "{kinds:?}");
    assert!(kinds.contains(&"table"), "{kinds:?}");
    assert!(kinds.contains(&"image"), "{kinds:?}");
    let paragraph = snapshot.blocks.iter().find_map(|b| match &b.kind {
        BlockKind::Paragraph { content } => Some(content.clone()),
        _ => None,
    }).unwrap();
    assert_eq!(paragraph.text, "Ghi chú quan trọng #kiem-tra");
    // Tag trích lại đúng sau import.
    let tags = store2.tags().unwrap();
    assert_eq!(tags[0].tag, "kiem-tra");
    let _ = Cell { row: 0, column: 0 };
}

#[test]
fn apple_notes_html_import_creates_notes() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("apple");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("note1.html"), "<html><head><title>Ghi chú Apple</title></head><body><div>Dòng đầu</div><div>Dòng sau</div></body></html>").unwrap();
    let mut store = Store::open(&temp.path().join("store")).unwrap();
    let imported = import_apple_notes_html(&mut store, &dir, None).unwrap();
    assert_eq!(imported.len(), 1);
    let editor = store.open_note(&imported[0]).unwrap();
    let text: String = editor.snapshot().blocks.iter().map(|b| match &b.kind {
        BlockKind::Paragraph { content } | BlockKind::Styled { content, .. } => content.text.clone(),
        _ => String::new(),
    }).collect::<Vec<_>>().join("\n");
    assert!(text.contains("Ghi chú Apple") && text.contains("Dòng đầu"), "{text}");
    let _ = Path::new(".");
}
