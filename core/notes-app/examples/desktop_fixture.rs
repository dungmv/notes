use notes_app::editor::{Batch, Editor, Operation};
use notes_app::offline::Store;
use rusqlite::{Connection, params};
use serde_json::json;
use std::{env, error::Error, fs, path::Path};

const M0: &str = include_str!("../../../fixtures/m0/editor-v1.json");
const PNG: &[u8] = include_bytes!("../../../fixtures/m1/pixel.png");

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let [command, directory] = args.as_slice() else {
        return Err(
            "usage: desktop_fixture <prepare|prepare-v1|hold-failures|release-text|release-image> <data-dir>"
                .into(),
        );
    };
    let root = Path::new(directory);
    match command.to_str() {
        Some("prepare") => prepare(root, false),
        Some("prepare-v1") => prepare(root, true),
        Some(command @ ("hold-failures" | "release-text" | "release-image")) => {
            let db = Connection::open_with_flags(
                root.join("notes.sqlite3"),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
            )?;
            if command == "hold-failures" {
                db.execute_batch(
                    "CREATE TRIGGER m2_fail_note BEFORE UPDATE OF crdt_state ON notes
                     BEGIN SELECT RAISE(ABORT, 'M2 fixture note write error'); END;
                     CREATE TRIGGER m2_fail_image BEFORE INSERT ON attachments
                     BEGIN SELECT RAISE(ABORT, 'M2 fixture attachment write error'); END;",
                )?;
            } else if command == "release-text" {
                db.execute_batch("DROP TRIGGER m2_fail_note;")?;
            } else {
                db.execute_batch("DROP TRIGGER m2_fail_image;")?;
            }
            Ok(())
        }
        _ => Err("unknown fixture command".into()),
    }
}

fn prepare(root: &Path, schema_v1: bool) -> Result<(), Box<dyn Error>> {
    fs::create_dir(root)?;
    let db_path = root.join("notes.sqlite3");
    let db = Connection::open(&db_path)?;
    db.execute_batch(include_str!("../src/offline/schema-v1.sql"))?;
    db.pragma_update(None, "user_version", 1)?;
    db.execute(
        "INSERT INTO folders(id,name,kind,created_at,updated_at)
         VALUES('desktop-folder','Công việc','normal',0,0)",
        [],
    )?;
    db.execute(
        "INSERT INTO notes(id,folder_id,crdt_state,title,plain_text_cache,created_at,updated_at)
         VALUES('desktop-v1','desktop-folder',?1,'Tiếng Việt 👩🏽‍💻 thử.','Tiếng Việt 👩🏽‍💻 thử.',0,0)",
        params![Editor::open(M0)?.crdt_state()],
    )?;
    drop(db);
    let mut store = Store::open(root)?;
    let target = store.create_note("inbox")?;
    let mut destination = store.open_note(&target)?;
    let target_block = destination.snapshot().blocks[0].id.clone();
    apply(
        &mut store,
        &mut destination,
        json!({
            "type":"replace", "id":target_block, "start":0, "end":0, "text":"Đích fixture"
        }),
    )?;
    let mut note = store.open_note("desktop-v1")?;
    store.paste_image(&mut note, 0, "intro".into(), 10, 10, PNG)?;
    let mut tail = note.snapshot().blocks[2].id.clone();
    for (id, label) in [("link-title", None), ("link-custom", Some("Nhãn riêng"))] {
        let next = format!("{id}-tail");
        apply(
            &mut store,
            &mut note,
            json!({
                "type":"insert_at", "id":tail, "start":0, "end":0, "tail_id":next,
                "block":{"id":id,"kind":"note_link","note_id":target,"label":label}
            }),
        )?;
        tail = next;
    }
    fs::write(
        root.join("expected-v1.json"),
        serde_json::to_vec_pretty(note.snapshot())?,
    )?;
    drop(store);
    if schema_v1 {
        // Recreate the schema-1 layout around the same CRDT state, references and blobs.
        let db = Connection::open(db_path)?;
        db.execute_batch(
            "BEGIN; DROP TABLE notes_fts; DROP TABLE preferences; PRAGMA user_version=1; COMMIT;",
        )?;
    }
    Ok(())
}

fn apply(
    store: &mut Store,
    note: &mut notes_app::offline::NoteEditor,
    operation: serde_json::Value,
) -> Result<(), Box<dyn Error>> {
    let operation: Operation = serde_json::from_value(operation)?;
    store.apply(
        note,
        Batch {
            revision: note.snapshot().revision,
            operations: vec![operation],
            external: false,
        },
    )?;
    Ok(())
}
