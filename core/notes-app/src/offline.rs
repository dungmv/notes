use crate::editor::{
    Batch, Block, BlockKind, Editor, EditorError, EditorUpdate, Operation, Snapshot, encode_state,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const ALL: &str = "all";
pub const INBOX: &str = "inbox";
pub const TRASH: &str = "trash";
const SCHEMA_VERSION: u32 = 3;
const NOTE_COLUMNS: &str = "n.id,n.folder_id,n.title,n.snippet,n.deleted_at IS NOT NULL";

#[derive(Debug, thiserror::Error)]
pub enum OfflineError {
    #[error("Dữ liệu SQLite: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("Tệp cục bộ: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Editor(#[from] EditorError),
    #[error("Dữ liệu dùng phiên bản chưa được hỗ trợ: {0}")]
    Schema(u32),
    #[error("Mục không còn tồn tại hoặc đã bị xóa")]
    Missing,
    #[error("Tên hoặc thao tác không hợp lệ")]
    Invalid,
}

#[derive(Debug, Serialize)]
pub struct Folder {
    id: String,
    name: String,
    kind: String,
}

#[derive(Debug, Serialize)]
pub struct Note {
    id: String,
    folder_id: String,
    title: String,
    snippet: String,
    deleted: bool,
}

pub struct Store {
    db: Connection,
    root: PathBuf,
}

pub struct NoteEditor {
    id: String,
    editor: Editor,
}

impl NoteEditor {
    pub fn snapshot(&self) -> &Snapshot {
        self.editor.snapshot()
    }
}

impl Store {
    pub fn open(root: &Path) -> Result<Self, OfflineError> {
        fs::create_dir_all(root)?;
        let mut db = Connection::open(root.join("notes.sqlite3"))?;
        db.busy_timeout(Duration::from_secs(3))?;
        // journal_mode returns a row, including when the requested mode is unavailable.
        let mode: String = db.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(OfflineError::Invalid);
        }
        db.execute_batch("PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")?;
        migrate(&mut db)?;
        Ok(Self {
            db,
            root: root.to_owned(),
        })
    }

    pub fn folders(&self) -> Result<Vec<Folder>, OfflineError> {
        let mut query = self
            .db
            .prepare("SELECT id,name,kind FROM folders ORDER BY sort_order,name,id")?;
        Ok(query
            .query_map([], |row| {
                Ok(Folder {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }

    pub fn checklist_sort(&self) -> Result<bool, OfflineError> {
        Ok(self.db.query_row(
            "SELECT auto_sort_checklist FROM preferences WHERE id=1",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn set_checklist_sort(&self, enabled: bool) -> Result<(), OfflineError> {
        changed(self.db.execute(
            "UPDATE preferences SET auto_sort_checklist=?1 WHERE id=1",
            [enabled],
        )?)
    }

    pub fn create_folder(&self, name: &str) -> Result<String, OfflineError> {
        let id = new_id();
        self.db.execute(
            "INSERT INTO folders(id,name,kind,sort_order,created_at,updated_at)
            VALUES(?1,?2,'normal',2,?3,?3)",
            params![id, valid_name(name)?, now()],
        )?;
        Ok(id)
    }

    pub fn rename_folder(&self, id: &str, name: &str) -> Result<(), OfflineError> {
        changed(self.db.execute(
            "UPDATE folders SET name=?2,updated_at=?3 WHERE id=?1 AND kind='normal'",
            params![id, valid_name(name)?, now()],
        )?)
    }

    pub fn notes(&self, folder: &str) -> Result<Vec<Note>, OfflineError> {
        self.query_notes(
            &format!(
                "SELECT {NOTE_COLUMNS} FROM notes n WHERE
                (?1='trash' AND n.deleted_at IS NOT NULL) OR
                (n.deleted_at IS NULL AND (?1='all' OR n.folder_id=?1))
                ORDER BY n.is_pinned DESC, coalesce(n.deleted_at,n.updated_at) DESC,n.id DESC"
            ),
            [folder],
        )
    }

    pub fn find(&self, input: &str) -> Result<Vec<Note>, OfflineError> {
        let normalized = fold(input);
        let expression = normalized
            .split(|ch: char| !ch.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(|word| format!("\"{word}\"*"))
            .collect::<Vec<_>>()
            .join(" AND ");
        if expression.is_empty() {
            return Ok(Vec::new());
        }
        self.query_notes(
            &format!(
                "SELECT {NOTE_COLUMNS} FROM notes_fts JOIN notes n ON n.id=notes_fts.note_id
                WHERE notes_fts MATCH ?1 AND n.deleted_at IS NULL AND n.is_locked=0
                ORDER BY rank,n.updated_at DESC,n.id"
            ),
            [&expression],
        )
    }

    fn query_notes(
        &self,
        sql: &str,
        values: impl rusqlite::Params,
    ) -> Result<Vec<Note>, OfflineError> {
        let mut query = self.db.prepare(sql)?;
        Ok(query
            .query_map(values, read_note)?
            .collect::<Result<_, _>>()?)
    }

    pub fn note(&self, id: &str) -> Result<Note, OfflineError> {
        self.db
            .query_row(
                &format!("SELECT {NOTE_COLUMNS} FROM notes n WHERE n.id=?1"),
                [id],
                read_note,
            )
            .optional()?
            .ok_or(OfflineError::Missing)
    }

    pub fn create_note(&mut self, folder: &str) -> Result<String, OfflineError> {
        let id = new_id();
        let editor = Editor::open(
            &serde_json::json!({
                "version": 1, "revision": 0,
                "blocks": [{"id": new_id(), "kind": "paragraph", "content": {"text": ""}}]
            })
            .to_string(),
        )?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_folder(&tx, folder)?;
        tx.execute(
            "INSERT INTO notes(id,folder_id,crdt_state,created_at,updated_at)
            VALUES(?1,?2,?3,?4,?4)",
            params![id, folder, editor.crdt_state(), now()],
        )?;
        reindex(&tx, &id)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn move_note(&self, id: &str, folder: &str) -> Result<(), OfflineError> {
        require_folder(&self.db, folder)?;
        changed(self.db.execute(
            "UPDATE notes SET folder_id=?2,updated_at=?3
            WHERE id=?1 AND deleted_at IS NULL",
            params![id, folder, now()],
        )?)
    }

    pub fn set_deleted(&mut self, id: &str, deleted: bool) -> Result<(), OfflineError> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let timestamp = now();
        let count = if deleted {
            tx.execute(
                "UPDATE notes SET deleted_at=?2,updated_at=?2 WHERE id=?1 AND deleted_at IS NULL",
                params![id, timestamp],
            )?
        } else {
            tx.execute(
                "UPDATE notes SET deleted_at=NULL,updated_at=?2,
                folder_id=CASE WHEN EXISTS(SELECT 1 FROM folders f WHERE f.id=notes.folder_id
                  AND f.kind IN ('normal','inbox') AND f.deleted_at IS NULL)
                  THEN folder_id ELSE 'inbox' END
                WHERE id=?1 AND deleted_at IS NOT NULL",
                params![id, timestamp],
            )?
        };
        changed(count)?;
        reindex(&tx, id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn open_note(&self, id: &str) -> Result<NoteEditor, OfflineError> {
        let (state, revision): (Vec<u8>, i64) = self
            .db
            .query_row(
                "SELECT crdt_state,revision FROM notes WHERE id=?1 AND is_locked=0",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(OfflineError::Missing)?;
        let editor = Editor::from_state(&state)?;
        if u64::try_from(revision).ok() != Some(editor.snapshot().revision) {
            return Err(EditorError::InvalidDocument.into());
        }
        Ok(NoteEditor {
            id: id.to_owned(),
            editor,
        })
    }

    pub fn apply(
        &mut self,
        note: &mut NoteEditor,
        batch: Batch,
    ) -> Result<EditorUpdate, OfflineError> {
        note.editor
            .apply_committing(batch, |snapshot, doc, update| {
                self.persist(&note.id, snapshot, &encode_state(doc), update, None)
            })
    }

    pub fn history(
        &mut self,
        note: &mut NoteEditor,
        revision: u64,
        redo: bool,
    ) -> Result<EditorUpdate, OfflineError> {
        note.editor
            .history_committing(revision, redo, |snapshot, doc, update| {
                self.persist(&note.id, snapshot, &encode_state(doc), update, None)
            })
    }

    pub fn paste_image(
        &mut self,
        note: &mut NoteEditor,
        revision: u64,
        id: String,
        start: u32,
        end: u32,
        png: &[u8],
    ) -> Result<EditorUpdate, OfflineError> {
        if !png.starts_with(b"\x89PNG\r\n\x1a\n") || png.len() > 32 * 1024 * 1024 {
            return Err(OfflineError::Invalid);
        }
        let hash = format!("{:x}", Sha256::digest(png));
        let attachment = Image {
            id: new_id(),
            hash,
            size: png.len() as i64,
        };
        let block = Block {
            id: new_id(),
            kind: BlockKind::Image {
                attachment_id: attachment.id.clone(),
            },
        };
        let operation = Operation::InsertAt {
            id,
            start,
            end,
            block,
            tail_id: new_id(),
        };
        note.editor.apply_committing(
            Batch {
                revision,
                operations: vec![operation],
                external: false,
            },
            |snapshot, doc, update| {
                self.persist(
                    &note.id,
                    snapshot,
                    &encode_state(doc),
                    update,
                    Some((attachment, png)),
                )
            },
        )
    }

    pub fn image_path(&self, note: &NoteEditor, id: &str) -> Result<PathBuf, OfflineError> {
        let hash: String = self
            .db
            .query_row(
                "SELECT content_hash FROM attachments WHERE id=?1 AND note_id=?2",
                params![id, note.id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(OfflineError::Missing)?;
        self.blob_path(&hash)
    }

    fn blob_path(&self, hash: &str) -> Result<PathBuf, OfflineError> {
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(OfflineError::Invalid);
        }
        Ok(self.root.join("blobs").join(&hash[..2]).join(hash))
    }

    fn persist(
        &mut self,
        note_id: &str,
        snapshot: &Snapshot,
        state: &[u8],
        update: &EditorUpdate,
        image: Option<(Image, &[u8])>,
    ) -> Result<(), OfflineError> {
        let text = plain_text(snapshot);
        let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
        let title: String = lines.next().unwrap_or_default().chars().take(160).collect();
        let snippet: String = lines
            .take(2)
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(240)
            .collect();
        let checklist = snapshot
            .blocks
            .iter()
            .any(|block| matches!(block.kind, BlockKind::Checklist { .. }));
        let blob = image
            .as_ref()
            .map(|(image, _)| self.blob_path(&image.hash))
            .transpose()?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let (Some((image, bytes)), Some(path)) = (image, blob) {
            write_blob(&path, bytes)?;
            tx.execute("INSERT INTO attachments(id,note_id,kind,file_name,mime_type,byte_size,content_hash,created_at)
                VALUES(?1,?2,'image','image.png','image/png',?3,?4,?5)",
                params![image.id, note_id, image.size, image.hash, now()])?;
        }
        let mut images = false;
        for block in &snapshot.blocks {
            if let BlockKind::Image { attachment_id } = &block.kind {
                images = true;
                if !tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM attachments WHERE id=?1 AND note_id=?2)",
                    params![attachment_id, note_id],
                    |row| row.get::<_, bool>(0),
                )? {
                    return Err(OfflineError::Missing);
                }
            }
        }
        if tx.execute(
            "UPDATE notes SET crdt_state=?1,revision=?2,title=?3,snippet=?4,
            plain_text_cache=?5,has_checklist=?6,has_attachments=?7,updated_at=?8
            WHERE id=?9 AND revision=?10 AND deleted_at IS NULL AND is_locked=0",
            params![
                state,
                sql_revision(update.revision)?,
                title,
                snippet,
                text,
                checklist,
                images,
                now(),
                note_id,
                sql_revision(update.base_revision)?
            ],
        )? != 1
        {
            return Err(EditorError::StaleRevision.into());
        }
        reindex(&tx, note_id)?;
        tx.commit()?;
        Ok(())
    }
}

struct Image {
    id: String,
    hash: String,
    size: i64,
}

fn sql_revision(revision: u64) -> Result<i64, OfflineError> {
    i64::try_from(revision).map_err(|_| OfflineError::Invalid)
}

fn write_blob(path: &Path, bytes: &[u8]) -> Result<(), OfflineError> {
    if path.is_file() {
        return Ok(());
    }
    let parent = path.parent().ok_or(OfflineError::Invalid)?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    publish_blob(temp, path)?;
    // Synchronize the directory entries before SQLite publishes the reference.
    #[cfg(unix)]
    for directory in parent.ancestors().take(3) {
        fs::File::open(directory)?.sync_all()?;
    }
    Ok(())
}

#[cfg(windows)]
fn publish_blob(temp: tempfile::NamedTempFile, path: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};

    let temp = temp.into_temp_path();
    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain([0])
            .collect::<Vec<_>>()
    };
    let source = wide(&temp);
    let destination = wide(path);
    // SAFETY: both paths are null-terminated and live until the call returns.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    } != 0
    {
        return Ok(());
    }
    Err(std::io::Error::last_os_error())
}

#[cfg(not(windows))]
fn publish_blob(temp: tempfile::NamedTempFile, path: &Path) -> std::io::Result<()> {
    temp.persist_noclobber(path)
        .map(|_| ())
        .map_err(|error| error.error)
}

fn read_note(row: &rusqlite::Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        folder_id: row.get(1)?,
        title: row.get(2)?,
        snippet: row.get(3)?,
        deleted: row.get(4)?,
    })
}

fn valid_name(name: &str) -> Result<&str, OfflineError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 160 {
        return Err(OfflineError::Invalid);
    }
    Ok(trimmed)
}

fn require_folder(db: &Connection, id: &str) -> Result<(), OfflineError> {
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM folders
        WHERE id=?1 AND kind IN ('normal','inbox') AND deleted_at IS NULL)",
        [id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(OfflineError::Missing)
    }
}

fn changed(count: usize) -> Result<(), OfflineError> {
    if count == 1 {
        Ok(())
    } else {
        Err(OfflineError::Missing)
    }
}

fn new_id() -> String {
    Uuid::now_v7().to_string()
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn plain_text(snapshot: &Snapshot) -> String {
    let mut lines = Vec::new();
    for block in &snapshot.blocks {
        match &block.kind {
            BlockKind::Paragraph { content }
            | BlockKind::Styled { content, .. }
            | BlockKind::Checklist { content, .. } => lines.push(content.text.as_str()),
            BlockKind::Table { rows } => {
                lines.extend(rows.iter().flatten().map(|cell| cell.text.as_str()))
            }
            BlockKind::NoteLink {
                label: Some(label), ..
            } => lines.push(label.as_str()),
            _ => (),
        }
    }
    lines.join("\n")
}

fn reindex(tx: &Transaction<'_>, id: &str) -> Result<(), OfflineError> {
    tx.execute("DELETE FROM notes_fts WHERE note_id=?1", [id])?;
    let content: Option<(String, String)> = tx.query_row(
        "SELECT title,plain_text_cache FROM notes WHERE id=?1 AND deleted_at IS NULL AND is_locked=0",
        [id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional()?;
    if let Some((title, body)) = content {
        tx.execute(
            "INSERT INTO notes_fts(note_id,title,body_folded) VALUES(?1,?2,?3)",
            params![id, fold(&title), fold(&body)],
        )?;
    }
    Ok(())
}

fn migrate(db: &mut Connection) -> Result<(), OfflineError> {
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: u32 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(OfflineError::Schema(version));
    }
    if version == 0 {
        tx.execute_batch(include_str!("offline/schema-v1.sql"))?;
    }
    if version < 2 {
        tx.execute_batch(include_str!("offline/schema-v2.sql"))?;
        let ids = {
            let mut query = tx.prepare("SELECT id FROM notes")?;
            query
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for id in ids {
            reindex(&tx, &id)?;
        }
    }
    if version < 3 {
        tx.execute_batch(include_str!("offline/schema-v3.sql"))?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    tx.commit()?;
    Ok(())
}

mod search;
use search::fold;
pub use search::{TextMatch, find_in_text};
