use notes_app::editor::{
    Batch, Block, BlockKind, Editor, EditorError, FIXTURE, Operation, ParagraphStyle, Style,
};
use notes_app::offline::{ALL, INBOX, OfflineError, Store, TRASH, TextMatch, find_in_text};
use rusqlite::{Connection, params};
use std::path::Path;

fn batch(revision: u64, operations: Vec<Operation>) -> Batch {
    Batch {
        revision,
        operations,
        external: false,
    }
}

fn replace(id: &str, start: u32, end: u32, text: &str) -> Operation {
    Operation::Replace {
        id: id.to_owned(),
        cell: None,
        start,
        end,
        text: text.to_owned(),
    }
}

fn db(root: &Path) -> Connection {
    Connection::open(root.join("notes.sqlite3")).unwrap()
}

#[test]
fn notes_search_and_lifecycle_survive_reopen() {
    let root = tempfile::tempdir().unwrap();
    let folder;
    let id;
    {
        let mut store = Store::open(root.path()).unwrap();
        assert_eq!(store.folders().unwrap().len(), 3);
        folder = store.create_folder("Công việc", None).unwrap();
        store.rename_folder(&folder, "Kế hoạch").unwrap();
        id = store.create_note(&folder).unwrap();
        let mut note = store.open_note(&id).unwrap();
        let block = note.snapshot().blocks[0].id.clone();
        let update = store
            .apply(
                &mut note,
                batch(
                    0,
                    vec![
                        replace(&block, 0, 0, "Đề xuất\n😀 Tiếng Việt"),
                        Operation::Format {
                            id: block,
                            cell: None,
                            start: 0,
                            end: 2,
                            style: Style::Bold,
                        },
                    ],
                ),
            )
            .unwrap();
        assert_eq!(update.revision, 1);
        for query in [
            "đề xuất",
            "de xuat",
            "Đe\u{302}\u{300} xuat",
            "tieng viet",
            "tiếng \"việt\"",
        ] {
            assert_eq!(store.find(query).unwrap().len(), 1, "{query}");
        }
        assert!(store.find(r#"*"() - "#).unwrap().is_empty());
        store.move_note(&id, INBOX).unwrap();
        assert!(store.notes(&folder).unwrap().is_empty());
        store.set_deleted(&id, true).unwrap();
        assert_eq!(store.notes(TRASH).unwrap().len(), 1);
        assert!(store.find("tiếng").unwrap().is_empty());
        store.set_deleted(&id, false).unwrap();
    }
    let store = Store::open(root.path()).unwrap();
    assert_eq!(store.notes(INBOX).unwrap().len(), 1);
    assert_eq!(store.find("tieng").unwrap().len(), 1);
    let note = store.open_note(&id).unwrap();
    let BlockKind::Paragraph { content } = &note.snapshot().blocks[0].kind else {
        panic!()
    };
    assert_eq!(content.text, "Đề xuất\n😀 Tiếng Việt");
    assert_eq!(content.marks[0].style, Style::Bold);
    assert_eq!(
        find_in_text("😀 Đe\u{302}\u{300} xuất", "de"),
        [TextMatch { start: 3, end: 7 }]
    );
}

#[test]
fn rollback_keeps_revision_history_and_index_and_accepts_retry() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    let id = store.create_note(INBOX).unwrap();
    let mut note = store.open_note(&id).unwrap();
    let mut other_store = Store::open(root.path()).unwrap();
    let mut concurrent_note = other_store.open_note(&id).unwrap();
    let block = note.snapshot().blocks[0].id.clone();
    let conn = db(root.path());
    conn.execute_batch(
        "CREATE TRIGGER fail_write BEFORE UPDATE OF crdt_state ON notes
        BEGIN SELECT RAISE(ABORT, 'test write error'); END;",
    )
    .unwrap();
    let change = batch(0, vec![replace(&block, 0, 0, "chưa lưu")]);
    assert!(store.apply(&mut note, change.clone()).is_err());
    assert_eq!(note.snapshot().revision, 0);
    assert_eq!(store.open_note(&id).unwrap().snapshot().revision, 0);
    assert!(store.find("chua").unwrap().is_empty());
    conn.execute_batch("DROP TRIGGER fail_write;").unwrap();
    assert_eq!(store.apply(&mut note, change).unwrap().revision, 1);
    assert!(matches!(
        other_store.apply(
            &mut concurrent_note,
            batch(0, vec![replace(&block, 0, 0, "x")])
        ),
        Err(OfflineError::Editor(EditorError::StaleRevision))
    ));
    assert_eq!(concurrent_note.snapshot().revision, 0);
    assert!(
        store
            .apply(&mut note, batch(0, vec![replace(&block, 0, 0, "x")]))
            .is_err()
    );
    assert_eq!(store.find("chua").unwrap().len(), 1);
    assert_eq!(store.history(&mut note, 1, false).unwrap().revision, 2);
    assert!(store.find("chua").unwrap().is_empty());
    assert_eq!(store.history(&mut note, 2, true).unwrap().revision, 3);
    assert_eq!(store.open_note(&id).unwrap().snapshot().revision, 3);
}

#[test]
fn paragraphs_and_checklists_keep_structure_and_marks_through_history() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    let id = store.create_note(INBOX).unwrap();
    let mut note = store.open_note(&id).unwrap();
    let head = note.snapshot().blocks[0].id.clone();
    let tail = "checklist".to_owned();
    store
        .apply(
            &mut note,
            batch(
                0,
                vec![
                    replace(&head, 0, 0, "Tiếng😀Việt"),
                    Operation::Format {
                        id: head.clone(),
                        cell: None,
                        start: 0,
                        end: 11,
                        style: Style::Underline,
                    },
                    Operation::ParagraphStyle {
                        id: head.clone(),
                        style: Some(ParagraphStyle::Title),
                        checklist: false,
                    },
                    Operation::SplitParagraph {
                        id: head.clone(),
                        offset: 7,
                        new_id: tail.clone(),
                    },
                    Operation::ParagraphStyle {
                        id: tail.clone(),
                        style: None,
                        checklist: true,
                    },
                    Operation::Indent {
                        id: tail.clone(),
                        delta: 1,
                    },
                    Operation::Toggle { id: tail.clone() },
                ],
            ),
        )
        .unwrap();
    let split = note.snapshot().blocks.clone();
    let BlockKind::Checklist {
        content,
        checked,
        depth,
    } = &split[1].kind
    else {
        panic!()
    };
    assert_eq!(content.text, "Việt");
    assert_eq!((content.marks[0].start, content.marks[0].end), (0, 4));
    assert_eq!((*checked, *depth), (true, 1));
    store
        .apply(
            &mut note,
            batch(1, vec![Operation::MergePrevious { id: tail }]),
        )
        .unwrap();
    let BlockKind::Styled { content, style } = &note.snapshot().blocks[0].kind else {
        panic!()
    };
    assert_eq!(
        (*style, content.text.as_str()),
        (ParagraphStyle::Title, "Tiếng😀Việt")
    );
    store.history(&mut note, 2, false).unwrap();
    assert_eq!(note.snapshot().blocks, split);
    assert_eq!(store.open_note(&id).unwrap().snapshot(), note.snapshot());
    store.history(&mut note, 3, true).unwrap();
    assert_eq!(note.snapshot().blocks.len(), 1);
}

#[test]
fn image_dedup_and_link_round_trip_and_undo() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    let first = store.create_note(INBOX).unwrap();
    let second = store.create_note(INBOX).unwrap();
    let mut a = store.open_note(&first).unwrap();
    let mut b = store.open_note(&second).unwrap();
    let block_a = a.snapshot().blocks[0].id.clone();
    let block_b = b.snapshot().blocks[0].id.clone();
    let png = include_bytes!("../../../fixtures/m1/pixel.png");
    store
        .apply(&mut a, batch(0, vec![replace(&block_a, 0, 0, "AB")]))
        .unwrap();
    let connection = db(root.path());
    connection
        .execute_batch(
            "CREATE TRIGGER fail_attachment BEFORE INSERT ON attachments
        BEGIN SELECT RAISE(ABORT, 'attachment write error'); END;",
        )
        .unwrap();
    assert!(
        store
            .paste_image(&mut a, 1, block_a.clone(), 1, 1, png)
            .is_err()
    );
    assert_eq!(store.open_note(&first).unwrap().snapshot(), a.snapshot());
    assert_eq!(a.snapshot().revision, 1);
    let hash = "42ff80f86ca28d848679ebfe6118a085c5d1ea9270ee3075be205e0d22bf673b";
    let orphan = root.path().join("blobs").join(&hash[..2]).join(hash);
    assert_eq!(std::fs::read(&orphan).unwrap(), png);
    let references: u32 = connection
        .query_row("SELECT count(*) FROM attachments", [], |row| row.get(0))
        .unwrap();
    assert_eq!(references, 0);
    connection
        .execute_batch("DROP TRIGGER fail_attachment")
        .unwrap();
    store.paste_image(&mut a, 1, block_a, 1, 1, png).unwrap();
    store.paste_image(&mut b, 0, block_b, 0, 0, png).unwrap();
    let image_id = |block: &Block| match &block.kind {
        BlockKind::Image { attachment_id } => attachment_id.clone(),
        _ => panic!("expected an image between paragraphs"),
    };
    let path_a = store
        .image_path(&a, &image_id(&a.snapshot().blocks[1]))
        .unwrap();
    let path_b = store
        .image_path(&b, &image_id(&b.snapshot().blocks[1]))
        .unwrap();
    assert_eq!(path_a, path_b);
    assert_eq!(path_a, orphan);
    assert_eq!(std::fs::read(&path_a).unwrap(), png);
    let tail = a.snapshot().blocks[2].id.clone();
    let update = store
        .apply(
            &mut a,
            batch(
                2,
                vec![Operation::InsertAt {
                    id: tail,
                    start: 0,
                    end: 0,
                    tail_id: "link-tail".into(),
                    block: Block {
                        id: "link".into(),
                        kind: BlockKind::NoteLink {
                            note_id: second.clone(),
                            label: None,
                        },
                    },
                }],
            ),
        )
        .unwrap();
    assert_eq!(update.revision, 3);
    store.history(&mut a, 3, false).unwrap();
    store.history(&mut a, 4, true).unwrap();
    store.set_deleted(&second, true).unwrap();
    assert!(
        serde_json::to_value(store.note(&second).unwrap()).unwrap()["deleted"]
            .as_bool()
            .unwrap()
    );
    store.set_deleted(&second, false).unwrap();
    assert_eq!(store.open_note(&first).unwrap().snapshot(), a.snapshot());
}

#[test]
fn v1_migrates_from_m0_document_and_rolls_back_on_sql_error() {
    let root = tempfile::tempdir().unwrap();
    let conn = db(root.path());
    conn.execute_batch(include_str!("../src/offline/schema-v1.sql"))
        .unwrap();
    conn.pragma_update(None, "user_version", 1).unwrap();
    let editor = Editor::open(FIXTURE).unwrap();
    conn.execute("INSERT INTO notes(id,folder_id,crdt_state,revision,title,plain_text_cache,created_at,updated_at)
        VALUES('m0','inbox',?1,0,'Đi họp','Kiểm thử',0,0)", params![editor.crdt_state()]).unwrap();
    conn.execute_batch("CREATE TABLE notes_fts (unexpected INTEGER);")
        .unwrap();
    assert!(Store::open(root.path()).is_err());
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 1);
    conn.execute_batch("DROP TABLE notes_fts;").unwrap();
    let store = Store::open(root.path()).unwrap();
    assert_eq!(store.notes(ALL).unwrap().len(), 1);
    assert_eq!(store.find("di hop").unwrap().len(), 1);
    assert_eq!(store.open_note("m0").unwrap().snapshot(), editor.snapshot());
    assert!(!store.checklist_sort().unwrap());
    store.set_checklist_sort(true).unwrap();
    assert!(Store::open(root.path()).unwrap().checklist_sort().unwrap());
}

#[test]
#[ignore = "reports local durable edit latency"]
fn profile_long_note() {
    use std::time::Instant;
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    let id = store.create_note(INBOX).unwrap();
    let mut note = store.open_note(&id).unwrap();
    let block = note.snapshot().blocks[0].id.clone();
    let text = "Đề xuất cho dự án tiếng Việt và thử nghiệm 😀\n".repeat(2_000);
    store
        .apply(&mut note, batch(0, vec![replace(&block, 0, 0, &text)]))
        .unwrap();
    let mut samples = Vec::new();
    for revision in 1..=40 {
        let started = Instant::now();
        store
            .apply(&mut note, batch(revision, vec![replace(&block, 0, 0, "a")]))
            .unwrap();
        samples.push(started.elapsed().as_secs_f64() * 1_000.0);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{} bytes, {} edits, p50 {:.2} ms, p95 {:.2} ms",
        text.len(),
        samples.len(),
        samples[19],
        samples[37]
    );
}

#[test]
fn tags_and_smart_folders_follow_content_and_lock() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(temp.path()).unwrap();

    let plain = store.create_note(INBOX).unwrap();
    let mut editor = store.open_note(&plain).unwrap();
    let block = editor.snapshot().blocks[0].id.clone();
    store.apply(&mut editor, batch(0, vec![replace(
        &block, 0, 0, "Họp nhóm #cong-viec và #hoc-tap")])).unwrap();

    let checklist = store.create_note(INBOX).unwrap();
    let mut editor = store.open_note(&checklist).unwrap();
    let block = editor.snapshot().blocks[0].id.clone();
    store.apply(&mut editor, batch(0, vec![
        replace(&block, 0, 0, "[] "),
        Operation::ParagraphStyle { id: block.clone(), style: None, checklist: true },
        replace(&block, 0, 0, "Chạy ca #cong-viec")])).unwrap();

    let tags = store.tags().unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].tag, "cong-viec");
    assert_eq!(tags[0].notes, 2);

    let note_ids = |notes: &Vec<notes_app::offline::Note>| -> Vec<String> {
        notes.iter().map(|note| {
            serde_json::to_value(note).unwrap()["id"].as_str().unwrap().to_owned()
        }).collect()
    };

    let found = note_ids(&store.find("#cong-viec").unwrap());
    assert_eq!(found.len(), 2);
    assert!(found.contains(&plain) && found.contains(&checklist));
    let hoc = note_ids(&store.find("#hoc-tap").unwrap());
    assert_eq!(hoc, vec![plain.clone()]);

    // Smart Folder: tag + checklist → chỉ ghi chú checklist.
    let smart = store
        .create_smart_folder(
            "Việc đang chạy",
            r#"{"tag":"cong-viec","has_checklist":true}"#,
        )
        .unwrap();
    let notes = note_ids(&store.smart_folder_notes(&smart).unwrap());
    assert_eq!(notes, vec![checklist.clone()]);
    let folder = store.folders().unwrap().into_iter().find(|f| f.id == smart).unwrap();
    assert!(folder.smart_query.is_some());

    // Gõ sai DSL bị từ chối.
    assert!(matches!(
        store.create_smart_folder("Lỗi", r#"{"tag":123}"#),
        Err(OfflineError::InvalidQuery)
    ));

    // Thư mục con: tạo trong thư mục thường, chặn cha là Smart Folder.
    let parent = store.create_folder("Cha", None).unwrap();
    let child = store.create_folder("Con", Some(&parent)).unwrap();
    assert!(store.folders().unwrap().iter().any(|f| f.id == child));
    assert!(store.create_folder("Sai", Some(&smart)).is_err());
}

#[test]
fn locking_scrubs_tags_and_remove_protection_restores() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = notes_app::offline::Store::open_with_key(
        temp.path(), Some(&[7u8; 32])).unwrap();
    let id = store.create_note(INBOX).unwrap();
    let mut editor = store.open_note(&id).unwrap();
    let block = editor.snapshot().blocks[0].id.clone();
    store.apply(&mut editor, batch(0, vec![replace(
        &block, 0, 0, "Ghi chú riêng tư
#cong-viec")])).unwrap();
    assert_eq!(store.tags().unwrap()[0].tag, "cong-viec");

    store.lock_note(&id, "mat-khau", None).unwrap();
    assert!(store.tags().unwrap().is_empty());
    assert!(store.find("#cong-viec").unwrap().is_empty());

    store.remove_protection(&id, "mat-khau").unwrap();
    assert_eq!(store.tags().unwrap()[0].tag, "cong-viec");
    let restored = serde_json::to_value(&store.find("#cong-viec").unwrap()[0]).unwrap();
    assert_eq!(restored["id"].as_str().unwrap(), id);
}
