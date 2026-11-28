use notes_app::editor::*;
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, ReadTxn, Transact, Update};

const OPS: &str = include_str!("../../../fixtures/m0/operations-v1.json");
const EXPECTED: &str = include_str!("../../../fixtures/m0/expected-v1.json");

fn apply(
    editor: &mut Editor,
    operations: Vec<Operation>,
    external: bool,
) -> Result<EditorUpdate, EditorError> {
    editor.apply(Batch {
        revision: editor.snapshot().revision,
        operations,
        external,
    })
}

fn replace(id: &str, start: u32, end: u32, text: &str) -> Operation {
    Operation::Replace {
        id: id.into(),
        cell: None,
        start,
        end,
        text: text.into(),
    }
}

#[test]
fn fixture_round_trip_and_incremental_patches() {
    let mut editor = Editor::open(FIXTURE).unwrap();
    let operations: Vec<Operation> = serde_json::from_str(OPS).unwrap();
    let mut rendered = editor.snapshot().blocks.clone();
    for (index, operation) in operations.into_iter().enumerate() {
        let update = apply(&mut editor, vec![operation], false).unwrap();
        assert_eq!(update.base_revision, index as u64);
        assert_eq!(update.revision, index as u64 + 1);
        assert_eq!(update.patches.len(), if index == 4 { 2 } else { 1 });
        for patch in update.patches {
            let Patch::Upsert { index, block } = patch else {
                panic!("unexpected delete")
            };
            if rendered.get(index).is_some_and(|old| old.id == block.id) {
                rendered[index] = block;
            } else {
                rendered.insert(index, block);
            }
        }
        assert_eq!(rendered, editor.snapshot().blocks);
    }
    let expected: Snapshot = serde_json::from_str(EXPECTED).unwrap();
    assert_eq!(editor.snapshot(), &expected);

    let serialized = serde_json::to_string(editor.snapshot()).unwrap();
    assert_eq!(Editor::open(&serialized).unwrap().snapshot(), &expected);

    let peer = Doc::new();
    peer.transact_mut()
        .apply_update(Update::decode_v1(&editor.crdt_state()).unwrap())
        .unwrap();
    let txn = peer.transact();
    let text = txn.get_text("block:intro").unwrap().get_string(&txn);
    assert_eq!(text, "Tiếng Việt 🙂 thử và á.");
}

#[test]
fn offsets_validate_and_reject_entire_batch() {
    let mut editor = Editor::open(FIXTURE).unwrap();
    let BlockKind::Paragraph { content } = &editor.snapshot().blocks[0].kind else {
        panic!()
    };
    let text = &content.text;
    let bytes = utf16_to_byte(text, 18).unwrap();
    assert_eq!(&text[..bytes], "Tiếng Việt 👩🏽‍💻");
    assert_eq!(byte_to_utf16(text, bytes), Ok(18));
    assert_eq!(
        byte_to_utf16(text, bytes - 1),
        Err(EditorError::InvalidRange)
    );
    assert_eq!(utf16_to_byte(text, 12), Err(EditorError::InvalidRange));
    let state = editor.crdt_state();
    for (start, end) in [(12, 13), (25, 26), (15, 13)] {
        let result = apply(
            &mut editor,
            vec![
                Operation::Toggle { id: "tasks".into() },
                replace("intro", start, end, "x"),
            ],
            false,
        );
        assert_eq!(result, Err(EditorError::InvalidRange));
        assert_eq!(editor.snapshot().revision, 0);
        assert_eq!(editor.crdt_state(), state);
    }
    assert_eq!(
        editor.apply(Batch {
            revision: 1,
            operations: vec![],
            external: false
        }),
        Err(EditorError::StaleRevision)
    );
}

#[test]
fn undo_redo_and_external_updates() {
    let mut editor = Editor::open(FIXTURE).unwrap();
    apply(&mut editor, vec![replace("intro", 11, 18, "🙂")], false).unwrap();
    apply(
        &mut editor,
        vec![Operation::Toggle { id: "tasks".into() }],
        true,
    )
    .unwrap();
    let tasks = editor.snapshot().blocks[1].clone();
    let undo = editor.history(2, false).unwrap();
    assert_eq!(undo.patches.len(), 1);
    assert_eq!(
        editor.snapshot().blocks[0],
        Editor::open(FIXTURE).unwrap().snapshot().blocks[0]
    );
    assert_eq!(editor.snapshot().blocks[1], tasks);
    assert_eq!(editor.history(3, true).unwrap().patches.len(), 1);
    apply(&mut editor, vec![replace("intro", 0, 0, "Một ")], true).unwrap();
    assert_eq!(editor.history(5, false), Err(EditorError::HistoryConflict));
}

#[test]
fn undo_redo_structural_edit_keeps_shifted_anchor() {
    let mut editor = Editor::open(FIXTURE).unwrap();
    let split = |id: &str, new_id: &str| Operation::SplitChecklist {
        id: id.into(),
        offset: 0,
        new_id: new_id.into(),
    };
    apply(
        &mut editor,
        vec![split("tasks-child", "child-split")],
        false,
    )
    .unwrap();
    editor.history(1, false).unwrap();
    apply(&mut editor, vec![split("tasks", "parent-split")], true).unwrap();
    editor.history(3, true).unwrap();
    let ids: Vec<_> = editor
        .snapshot()
        .blocks
        .iter()
        .map(|block| block.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "intro",
            "tasks",
            "parent-split",
            "tasks-child",
            "child-split",
            "table"
        ]
    );
}

fn table_document() -> String {
    let cell = |text: &str| serde_json::json!({"text": text, "marks": []}).to_string();
    format!(
        r#"{{"version":1,"revision":0,"blocks":[{{"id":"tbl","kind":"table",
        "rows":[[{c00},{c01}],[{c10},{c11}]]}}]}}"#,
        c00 = cell("Một"),
        c01 = cell("Hai"),
        c10 = cell("Ba"),
        c11 = cell("Bốn"),
    )
}

fn table_op(operation: serde_json::Value) -> Operation {
    serde_json::from_value(operation).unwrap()
}

fn table_rows(editor: &Editor) -> Vec<Vec<RichText>> {
    match &editor.snapshot().blocks[0].kind {
        BlockKind::Table { rows } => rows.clone(),
        _ => panic!("block is not a table"),
    }
}

#[test]
fn table_structure_operations_keep_cells_and_history() {
    let mut editor = Editor::open(&table_document()).unwrap();
    apply(&mut editor, vec![table_op(serde_json::json!({
        "type": "table_insert_row", "id": "tbl", "after_row": 0 }))], false).unwrap();
    apply(&mut editor, vec![table_op(serde_json::json!({
        "type": "table_insert_column", "id": "tbl", "after_column": null }))], false).unwrap();
    let rows = table_rows(&editor);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].len(), 3);

    // Sửa ô sau khi đổi cấu trúc: khóa vị trí phải khớp CRDT.
    apply(&mut editor, vec![Operation::Replace {
        id: "tbl".into(),
        cell: Some(Cell { row: 2, column: 1 }),
        start: 0,
        end: 3,
        text: "Sửa 😀".into(),
    }], false).unwrap();
    assert_eq!(table_rows(&editor)[2][1].text, "Sửa 😀");

    apply(&mut editor, vec![table_op(serde_json::json!({
        "type": "table_move_row", "id": "tbl", "from": 0, "to": 2 }))], false).unwrap();
    assert_eq!(table_rows(&editor)[2][0].text, "Một");
    apply(&mut editor, vec![table_op(serde_json::json!({
        "type": "table_move_column", "id": "tbl", "from": 1, "to": 0 }))], false).unwrap();
    assert_eq!(table_rows(&editor)[2][0].text, "Hai");

    apply(&mut editor, vec![table_op(serde_json::json!({
        "type": "table_remove_row", "id": "tbl", "row": 0 }))], false).unwrap();
    apply(&mut editor, vec![table_op(serde_json::json!({
        "type": "table_remove_column", "id": "tbl", "column": 1 }))], false).unwrap();
    let rows = table_rows(&editor);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1][0].text, "Hai");
    assert_eq!(rows[0][0].text, "Sửa 😀");

    // Undo toàn bộ chuỗi cấu trúc (7 op) rồi redo: nội dung ô quay về đúng.
    for _ in 0..7 {
        editor.history(editor.snapshot().revision, false).unwrap();
    }
    let rows = table_rows(&editor);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0].text, "Một");
    assert_eq!(rows[1][1].text, "Bốn");
    for _ in 0..7 {
        editor.history(editor.snapshot().revision, true).unwrap();
    }
    let rows = table_rows(&editor);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0].text, "Sửa 😀");
    assert_eq!(rows[1][0].text, "Hai");
}

#[test]
fn row_insert_realigned_positional_keys_for_second_editor() {
    let mut first = Editor::open(&table_document()).unwrap();
    let mut second = Editor::from_state(&first.crdt_state()).unwrap();
    apply(&mut first, vec![table_op(serde_json::json!({
        "type": "table_insert_row", "id": "tbl", "after_row": null }))], false).unwrap();
    apply(&mut second, vec![Operation::Replace {
        id: "tbl".into(),
        cell: Some(Cell { row: 1, column: 1 }),
        start: 0,
        end: 0,
        text: "Từ thiết bị khác".into(),
    }], false).unwrap();

    // Trạng thái của máy một (đã chèn hàng) dựng lại phải giữ nội dung ô cũ
    // và cho máy hai sửa tiếp đúng vị trí mới.
    let merged = Editor::from_state(&first.crdt_state()).unwrap();
    let rows = table_rows(&merged);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][1].text, "Hai");
    assert_eq!(rows[1][1].text, "Bốn");
    let _ = second;
}

#[test]
fn insert_at_creates_table_and_keeps_cell_content() {
    let mut editor = Editor::open(FIXTURE).unwrap();
    let revision = editor.snapshot().revision;
    let paragraph = editor.snapshot().blocks[0].id.clone();
    let table = serde_json::json!({
        "id": "new-table", "kind": "table",
        "rows": [[{"text":"A","marks":[]},{"text":"B","marks":[]}]]
    });
    let update = apply(&mut editor, vec![table_op(serde_json::json!({
        "type": "insert_at", "id": paragraph, "start": 0, "end": 0,
        "block": table, "tail_id": "tail-1" }))], false).unwrap();
    assert_eq!(update.revision, revision + 1);
    let kinds: Vec<&str> = editor.snapshot().blocks.iter()
        .map(|block| match &block.kind {
            BlockKind::Table { .. } => "table",
            _ => "other",
        }).collect();
    assert!(kinds.contains(&"table"));
    let state = Editor::from_state(&editor.crdt_state()).unwrap();
    let block = state.snapshot().blocks.iter().find(|b| b.id == "new-table").unwrap();
    let BlockKind::Table { rows } = &block.kind else { panic!("not a table") };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "A");
}
