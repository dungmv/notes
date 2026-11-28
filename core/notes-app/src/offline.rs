use crate::editor::{
    Batch, Block, BlockKind, Editor, EditorError, EditorUpdate, Operation, Snapshot, encode_state,
};
use locked::LockedSession;
use notes_crypto::notelock::LockedAttachment;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const ALL: &str = "all";
pub const INBOX: &str = "inbox";
pub const TRASH: &str = "trash";
const SCHEMA_VERSION: u32 = 6;
const NOTE_COLUMNS: &str = "n.id,n.folder_id,n.title,n.snippet,n.deleted_at IS NOT NULL,n.is_locked";

#[derive(Debug, thiserror::Error)]
pub enum OfflineError {
    #[error("Câu truy vấn Smart Folder không hợp lệ")]
    InvalidQuery,
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
    #[error("Ghi chú đang ở trạng thái khóa")]
    NoteLocked,
    #[error("Mật khẩu khóa không đúng")]
    WrongPassphrase,
    #[error("Tính năng khóa ghi chú yêu cầu kho dữ liệu mã hóa SQLCipher")]
    LockUnsupported,
    #[error("Không thể khóa ghi chú khi đồng bộ đang bật (TD-003)")]
    SyncStillEnabled,
    #[error("Khóa dữ liệu cục bộ không đúng hoặc còn thiếu")]
    InvalidDbKey,
    #[error("Mã hóa Locked Notes: {0}")]
    Crypto(#[from] notes_crypto::CryptoError),
    #[error("Dữ liệu Locked Notes không hợp lệ: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Serialize)]
pub struct Folder {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub smart_query: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SmartQuery {
    pub tag: Option<String>,
    pub has_checklist: Option<bool>,
    pub has_attachments: Option<bool>,
    pub folder: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TagCount {
    pub tag: String,
    pub notes: i64,
}

#[derive(Debug, Serialize)]
pub struct Note {
    id: String,
    folder_id: String,
    title: String,
    snippet: String,
    deleted: bool,
    locked: bool,
}

impl Note {
    pub fn title(&self) -> &str {
        &self.title
    }
}

pub struct Store {
    db: Connection,
    root: PathBuf,
    encrypted: bool,
}

/// Attachment mang theo khi import: blob được ghi bền vững trước khi commit
/// tham chiếu, đúng chính sách lưu trữ 02 §3.1.
pub struct ImportedAttachment {
    pub id: String,
    pub content_hash: String,
    pub bytes: Vec<u8>,
    pub file_name: String,
}

impl Store {
    /// Tạo ghi chú mới với tài liệu khối cho trước (dùng bởi import).
    pub fn create_note_with_blocks(
        &mut self,
        folder: &str,
        blocks: Vec<Block>,
        attachments: Vec<ImportedAttachment>,
    ) -> Result<String, OfflineError> {
        for block in &blocks {
            if let BlockKind::Image { attachment_id } = &block.kind {
                if !attachments.iter().any(|att| &att.id == attachment_id) {
                    return Err(OfflineError::Missing);
                }
            }
        }
        for attachment in &attachments {
            self.write_blob_bytes(&attachment.content_hash, &attachment.bytes)?;
        }
        let json = serde_json::json!({"version": 1, "revision": 0, "blocks": blocks});
        let editor = Editor::open(&json.to_string())?;
        let state = editor.crdt_state();
        let id = new_id();
        let ts = now();
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_folder(&tx, folder)?;
        tx.execute(
            "INSERT INTO notes(id,folder_id,crdt_state,revision,created_at,updated_at)
            VALUES(?1,?2,?3,0,?4,?4)",
            params![id, folder, state, ts],
        )?;
        for attachment in &attachments {
            tx.execute(
                "INSERT INTO attachments(id,note_id,kind,file_name,mime_type,byte_size,content_hash,created_at)
                VALUES(?1,?2,'image',?3,'image/png',?4,?5,?6)",
                params![
                    attachment.id,
                    id,
                    attachment.file_name,
                    attachment.bytes.len() as i64,
                    attachment.content_hash,
                    ts
                ],
            )?;
        }
        reindex(&tx, &id)?;
        update_tags(&tx, &id, &plain_text(editor.snapshot()))?;
        crate::sync::enqueue_note_create(&tx, &id, folder, &state, ts, ts)?;
        tx.commit()?;
        Ok(id)
    }
}

pub struct NoteEditor {
    id: String,
    editor: Editor,
    pub(super) lock: Option<LockedSession>,
}

impl NoteEditor {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn snapshot(&self) -> &Snapshot {
        self.editor.snapshot()
    }

    pub fn is_locked(&self) -> bool {
        self.lock.is_some()
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
            encrypted: false,
        })
    }

    pub fn folders(&self) -> Result<Vec<Folder>, OfflineError> {
        let mut query = self.db.prepare(
            "SELECT id,parent_id,name,kind,smart_query_json FROM folders
            ORDER BY sort_order,name,id",
        )?;
        Ok(query
            .query_map([], |row| {
                let json: Option<String> = row.get(4)?;
                Ok(Folder {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    name: row.get(2)?,
                    kind: row.get(3)?,
                    smart_query: json
                        .and_then(|text| serde_json::from_str(&text).ok()),
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

    pub fn create_folder(&mut self, name: &str, parent: Option<&str>) -> Result<String, OfflineError> {
        let id = new_id();
        let ts = now();
        let valid = valid_name(name)?.to_owned();
        let parent_id = match parent {
            None => None,
            Some(value) if value == "inbox" || value == "all" => None,
            Some(value) => {
                let exists: bool = self.db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM folders
                    WHERE id=?1 AND kind='normal' AND deleted_at IS NULL AND smart_query_json IS NULL)",
                    [value],
                    |row| row.get(0),
                )?;
                if !exists {
                    return Err(OfflineError::Missing);
                }
                Some(value.to_owned())
            }
        };
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO folders(id,parent_id,name,kind,sort_order,created_at,updated_at)
            VALUES(?1,?2,?3,'normal',2,?4,?4)",
            params![id, parent_id, valid, ts],
        )?;
        crate::sync::enqueue_folder_upsert(&tx, &id, &valid, "normal", 2, ts, ts)?;
        tx.commit()?;
        Ok(id)
    }

    /// Tạo Smart Folder: lưu JSON DSL vào `smart_query_json`, loại thư mục
    /// thường được đánh giá theo [02 §1.1](../specs/02-data-model-and-storage.md).
    pub fn create_smart_folder(&mut self, name: &str, query: &str) -> Result<String, OfflineError> {
        let parsed: SmartQuery = serde_json::from_str(query).map_err(|_| OfflineError::InvalidQuery)?;
        if parsed.tag.is_none()
            && parsed.has_checklist.is_none()
            && parsed.has_attachments.is_none()
            && parsed.folder.is_none()
        {
            return Err(OfflineError::InvalidQuery);
        }
        let id = new_id();
        let ts = now();
        let valid = valid_name(name)?.to_owned();
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO folders(id,parent_id,name,kind,smart_query_json,sort_order,created_at,updated_at)
            VALUES(?1,NULL,?2,'normal',?3,2,?4,?4)",
            params![id, valid, serde_json::to_string(&parsed)?, ts],
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Danh sách ghi chú khớp điều kiện Smart Folder; bỏ qua ghi chú khóa và
    /// đã xóa. `folder` trong DSL giới hạn theo thư mục cha cụ thể.
    pub fn smart_folder_notes(&self, id: &str) -> Result<Vec<Note>, OfflineError> {
        let json: Option<String> = self
            .db
            .query_row(
                "SELECT smart_query_json FROM folders WHERE id=?1 AND deleted_at IS NULL",
                [id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let Some(json) = json else {
            return Err(OfflineError::Missing);
        };
        let query: SmartQuery =
            serde_json::from_str(&json).map_err(|_| OfflineError::InvalidQuery)?;
        let mut sql = format!(
            "SELECT {NOTE_COLUMNS} FROM notes n WHERE n.deleted_at IS NULL AND n.is_locked=0"
        );
        if query.folder.is_some() {
            sql.push_str(" AND n.folder_id=?1");
        }
        if query.has_checklist.is_some() {
            sql.push_str(" AND n.has_checklist=?2");
        }
        if query.has_attachments.is_some() {
            sql.push_str(" AND n.has_attachments=?3");
        }
        if query.tag.is_some() {
            sql.push_str(
                " AND EXISTS(SELECT 1 FROM note_tags t WHERE t.note_id=n.id AND t.tag=?4)",
            );
        }
        sql.push_str(" ORDER BY n.is_pinned DESC, n.updated_at DESC, n.id DESC");
        self.query_notes(
            &sql,
            params![
                query.folder,
                query.has_checklist,
                query.has_attachments,
                query.tag
            ],
        )
    }

    /// Danh sách tag đang dùng kèm số ghi chú thường, sắp theo số lượng giảm dần.
    pub fn tags(&self) -> Result<Vec<TagCount>, OfflineError> {
        let mut query = self.db.prepare(
            "SELECT t.tag, COUNT(*) FROM note_tags t JOIN notes n ON n.id=t.note_id
            WHERE n.deleted_at IS NULL AND n.is_locked=0
            GROUP BY t.tag ORDER BY COUNT(*) DESC, t.tag",
        )?;
        Ok(query
            .query_map([], |row| {
                Ok(TagCount {
                    tag: row.get(0)?,
                    notes: row.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }

    pub fn rename_folder(&mut self, id: &str, name: &str) -> Result<(), OfflineError> {
        let ts = now();
        let valid = valid_name(name)?.to_owned();
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        changed(tx.execute(
            "UPDATE folders SET name=?2,updated_at=?3 WHERE id=?1 AND kind='normal'",
            params![id, valid, ts],
        )?)?;
        let (kind, sort_order, created_at): (String, i64, i64) = tx.query_row(
            "SELECT kind, sort_order, created_at FROM folders WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        crate::sync::enqueue_folder_upsert(&tx, id, &valid, &kind, sort_order, created_at, ts)?;
        tx.commit()?;
        Ok(())
    }

    pub fn notes(&self, folder: &str) -> Result<Vec<Note>, OfflineError> {
        let smart: Option<String> = self
            .db
            .query_row(
                "SELECT smart_query_json FROM folders WHERE id=?1",
                [folder],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        if smart.is_some() {
            return self.smart_folder_notes(folder);
        }
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
        let words: Vec<String> = normalized
            .split(|ch: char| !ch.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(|word| format!("\"{word}\"*"))
            .collect();
        let mut results = Vec::new();
        if !words.is_empty() {
            let expression = words.join(" AND ");
            results.extend(self.query_notes(
                &format!(
                    "SELECT {NOTE_COLUMNS} FROM notes_fts JOIN notes n ON n.id=notes_fts.note_id
                    WHERE notes_fts MATCH ?1 AND n.deleted_at IS NULL AND n.is_locked=0
                    ORDER BY rank,n.updated_at DESC,n.id"
                ),
                [&expression],
            )?);
        }
        // Ghi chú khóa chỉ lộ tiêu đề trong tìm kiếm, qua cột notes.title
        // riêng theo chính sách lưu trữ 02 §3.2; FTS không giữ bản ghi của chúng.
        let terms: Vec<String> = normalized
            .split(|ch: char| !ch.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_owned)
            .collect();
        if !terms.is_empty() {
            let mut locked = self.query_notes(
                &format!(
                    "SELECT {NOTE_COLUMNS} FROM notes n
                    WHERE n.is_locked=1 AND n.deleted_at IS NULL
                    ORDER BY n.updated_at DESC,n.id"
                ),
                [],
            )?;
            locked.retain(|note| {
                let title = fold(&note.title);
                terms.iter().all(|term| title.contains(term))
            });
            results.extend(locked);
        }
        // Token `#tag` trong truy vấn khớp bảng note_tags (02 §1.3).
        let tag_terms: Vec<String> = input
            .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
            .filter_map(|word| word.strip_prefix('#'))
            .filter(|tag| !tag.is_empty())
            .map(str::to_owned)
            .collect();
        if !tag_terms.is_empty() {
            let placeholders = tag_terms
                .iter()
                .enumerate()
                .map(|(index, _)| format!("?{}", index + 1))
                .collect::<Vec<_>>()
                .join(",");
            let mut tagged = self.query_notes(
                &format!(
                    "SELECT {NOTE_COLUMNS} FROM notes n WHERE n.deleted_at IS NULL AND n.is_locked=0
                    AND EXISTS(SELECT 1 FROM note_tags t WHERE t.note_id=n.id AND t.tag IN ({placeholders}))
                    ORDER BY n.updated_at DESC,n.id"
                ),
                rusqlite::params_from_iter(tag_terms.iter()),
            )?;
            let seen: std::collections::HashSet<&str> =
                results.iter().map(|note| note.id.as_str()).collect();
            tagged.retain(|note| !seen.contains(note.id.as_str()));
            results.extend(tagged);
        }
        Ok(results)
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
        let ts = now();
        let state = editor.crdt_state();
        tx.execute(
            "INSERT INTO notes(id,folder_id,crdt_state,created_at,updated_at)
            VALUES(?1,?2,?3,?4,?4)",
            params![id, folder, state, ts],
        )?;
        reindex(&tx, &id)?;
        crate::sync::enqueue_note_create(&tx, &id, folder, &state, ts, ts)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn move_note(&mut self, id: &str, folder: &str) -> Result<(), OfflineError> {
        let ts = now();
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_folder(&tx, folder)?;
        changed(tx.execute(
            "UPDATE notes SET folder_id=?2,updated_at=?3
            WHERE id=?1 AND deleted_at IS NULL",
            params![id, folder, ts],
        )?)?;
        if !note_locked(&tx, id)? {
            crate::sync::enqueue_note_move(&tx, id, folder, ts)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_pinned(&mut self, id: &str, pinned: bool) -> Result<(), OfflineError> {
        let ts = now();
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        changed(tx.execute(
            "UPDATE notes SET is_pinned=?2,updated_at=?3
            WHERE id=?1 AND deleted_at IS NULL",
            params![id, pinned, ts],
        )?)?;
        if !note_locked(&tx, id)? {
            crate::sync::enqueue_note_pin(&tx, id, pinned, ts)?;
        }
        tx.commit()?;
        Ok(())
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
        if !note_locked(&tx, id)? {
            crate::sync::enqueue_note_delete(&tx, id, if deleted { Some(timestamp) } else { None }, timestamp)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn open_note(&self, id: &str) -> Result<NoteEditor, OfflineError> {
        let (state, revision, locked): (Vec<u8>, i64, i64) = self
            .db
            .query_row(
                "SELECT crdt_state,revision,is_locked FROM notes WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(OfflineError::Missing)?;
        if locked != 0 {
            return Err(OfflineError::NoteLocked);
        }
        let editor = Editor::from_state(&state)?;
        if u64::try_from(revision).ok() != Some(editor.snapshot().revision) {
            return Err(EditorError::InvalidDocument.into());
        }
        Ok(NoteEditor {
            id: id.to_owned(),
            editor,
            lock: None,
        })
    }

    pub fn apply(
        &mut self,
        note: &mut NoteEditor,
        batch: Batch,
    ) -> Result<EditorUpdate, OfflineError> {
        note.editor
            .apply_committing(batch, |snapshot, doc, update| {
                self.persist(&note.id, note.lock.as_mut(), snapshot, &encode_state(doc), update, None)
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
                self.persist(&note.id, note.lock.as_mut(), snapshot, &encode_state(doc), update, None)
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
                    note.lock.as_mut(),
                    snapshot,
                    &encode_state(doc),
                    update,
                    Some((attachment, png)),
                )
            },
        )
    }

    pub fn image_path(&self, note: &NoteEditor, id: &str) -> Result<PathBuf, OfflineError> {
        if note.lock.is_some() {
            return Err(OfflineError::NoteLocked);
        }
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
        lock: Option<&mut LockedSession>,
        snapshot: &Snapshot,
        state: &[u8],
        update: &EditorUpdate,
        image: Option<(Image, &[u8])>,
    ) -> Result<(), OfflineError> {
        let text = plain_text(snapshot);
        let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
        let title: String = lines.next().unwrap_or_default().chars().take(160).collect();
        if let Some(lock) = lock {
            return self.persist_locked(
                note_id, lock, snapshot, &title, state, update, image,
            );
        }
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
        let ts = now();
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
                ts,
                note_id,
                sql_revision(update.base_revision)?
            ],
        )? != 1
        {
            return Err(EditorError::StaleRevision.into());
        }
        reindex(&tx, note_id)?;
        update_tags(&tx, note_id, &text)?;
        crate::sync::enqueue_note_crdt(&tx, note_id, state, ts)?;
        tx.commit()?;
        Ok(())
    }

    /// Lưu thay đổi trong phiên mở của ghi chú khóa: trạng thái và manifest
    /// được mã hóa lại bằng NLK, tiêu đề được cập nhật (được phép hiển thị),
    /// còn snippet/FTS/cache/cờ suy ra giữ nguyên dạng đã scrub trên đĩa.
    /// Không enqueue outbox — Locked Notes chưa đồng bộ (`TD-003`).
    fn persist_locked(
        &mut self,
        note_id: &str,
        lock: &mut LockedSession,
        snapshot: &Snapshot,
        title: &str,
        state: &[u8],
        update: &EditorUpdate,
        image: Option<(Image, &[u8])>,
    ) -> Result<(), OfflineError> {
        let ts = now();
        // Giữ manifest khớp với các ảnh còn tham chiếu trong tài liệu sau khi
        // gỡ embed; blob mã hóa mồ côi được dọn theo TD-012.
        let referenced: std::collections::HashSet<&str> = snapshot
            .blocks
            .iter()
            .filter_map(|block| match &block.kind {
                BlockKind::Image { attachment_id } => Some(attachment_id.as_str()),
                _ => None,
            })
            .collect();
        lock.manifest
            .attachments
            .retain(|entry| referenced.contains(entry.id.as_str()));
        if let Some((image, bytes)) = image {
            let key = notes_crypto::notelock::generate_image_blob_key()?;
            let blob_id = notes_crypto::notelock::generate_blob_id();
            let ciphertext = notes_crypto::notelock::encrypt_locked_blob(&blob_id, &key, bytes)?;
            let path = self.blob_path(&blob_id)?;
            write_blob(&path, &ciphertext)?;
            lock.manifest
                .attachments
                .push(LockedAttachment {
                    id: image.id,
                    kind: "image".to_owned(),
                    file_name: "image.png".to_owned(),
                    mime_type: "image/png".to_owned(),
                    byte_size: image.size,
                    content_hash: image.hash,
                    blob_id,
                    wrapped_key_hex: notes_crypto::notelock::wrap_image_blob_key(
                        note_id,
                        lock.nlk(),
                        &key,
                    )?,
                    created_at: ts,
                });
        }
        let state_envelope = notes_crypto::notelock::encrypt_lock_payload(
            lock.nlk().as_bytes(),
            note_id,
            notes_crypto::notelock::LOCK_CONTEXT_STATE,
            state,
        )?;
        let manifest_envelope = notes_crypto::notelock::encrypt_lock_payload(
            lock.nlk().as_bytes(),
            note_id,
            notes_crypto::notelock::LOCK_CONTEXT_MANIFEST,
            &serde_json::to_vec(&lock.manifest)?,
        )?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute(
            "UPDATE notes SET crdt_state=?1,revision=?2,title=?3,lock_manifest=?4,updated_at=?5
            WHERE id=?6 AND revision=?7 AND deleted_at IS NULL AND is_locked=1",
            params![
                state_envelope,
                sql_revision(update.revision)?,
                title,
                manifest_envelope,
                ts,
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
        locked: row.get::<_, i64>(5)? != 0,
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

fn note_locked(db: &Connection, id: &str) -> Result<bool, OfflineError> {
    Ok(db.query_row(
        "SELECT is_locked FROM notes WHERE id=?1",
        [id],
        |row| row.get::<_, i64>(0),
    )? != 0)
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
        let tags: Vec<String> = {
            let mut query = tx.prepare("SELECT tag FROM note_tags WHERE note_id=?1")?;
            query
                .query_map([id], |row| row.get(0))?
                .collect::<Result<_, _>>()?
        };
        tx.execute(
            "INSERT INTO notes_fts(note_id,title,body_folded,tags) VALUES(?1,?2,?3,?4)",
            params![id, fold(&title), fold(&body), tags.join(" ")],
        )?;
    }
    Ok(())
}

/// Trích xuất tag inline `#từ-khóa` (Unicode) từ văn bản thuần của ghi chú.
fn extract_tags(text: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for word in text.split(|ch: char| {
        ch.is_whitespace() || matches!(ch, ',' | ';' | '.' | '!' | '?' | '(' | ')' | '"')
    }) {
        let Some(rest) = word.strip_prefix('#') else {
            continue;
        };
        let tag: String = rest
            .chars()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '-')
            .collect();
        if !tag.is_empty() && !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}

fn update_tags(tx: &Transaction<'_>, id: &str, text: &str) -> Result<(), OfflineError> {
    tx.execute("DELETE FROM note_tags WHERE note_id=?1", [id])?;
    for tag in extract_tags(text) {
        tx.execute(
            "INSERT OR IGNORE INTO note_tags(note_id,tag) VALUES(?1,?2)",
            params![id, tag],
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
        // FTS được nạp lại toàn bộ ở bước v6 (sau khi note_tags tồn tại).
    }
    if version < 3 {
        tx.execute_batch(include_str!("offline/schema-v3.sql"))?;
    }
    if version < 4 {
        tx.execute_batch(include_str!("offline/schema-v4.sql"))?;
        tx.execute("INSERT INTO sync_config(id, device_id) VALUES(1, ?1)", [new_id()])?;
    }
    if version < 5 {
        tx.execute_batch(include_str!("offline/schema-v5.sql"))?;
    }
    if version < 6 {
        tx.execute_batch(include_str!("offline/schema-v6.sql"))?;
        let ids = {
            let mut query = tx.prepare("SELECT id FROM notes WHERE deleted_at IS NULL AND is_locked=0")?;
            query
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for id in ids {
            let text: String = tx.query_row(
                "SELECT plain_text_cache FROM notes WHERE id=?1",
                [&id],
                |row| row.get(0),
            )?;
            update_tags(&tx, &id, &text)?;
            reindex(&tx, &id)?;
        }
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    tx.commit()?;
    Ok(())
}

impl Store {
    pub(crate) fn db(&self) -> &Connection {
        &self.db
    }

    pub(crate) fn db_mut(&mut self) -> &mut Connection {
        &mut self.db
    }

    pub(crate) fn write_blob_bytes(&self, hash: &str, bytes: &[u8]) -> Result<(), OfflineError> {
        let path = self.blob_path(hash)?;
        write_blob(&path, bytes)
    }

    pub(crate) fn read_blob_bytes(&self, hash: &str) -> Result<Vec<u8>, OfflineError> {
        let path = self.blob_path(hash)?;
        Ok(fs::read(path)?)
    }
}

pub(crate) fn summarize_note_snapshot(snapshot: &Snapshot) -> (String, String, String, bool, bool) {
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
    let images = snapshot
        .blocks
        .iter()
        .any(|block| matches!(block.kind, BlockKind::Image { .. }));
    (title, snippet, text, checklist, images)
}

pub(crate) fn reindex_tx(tx: &Transaction<'_>, id: &str) -> Result<(), OfflineError> {
    reindex(tx, id)
}

pub(crate) fn now_ms() -> i64 {
    now()
}

pub(crate) fn new_uuid_v7() -> String {
    new_id()
}

mod locked;
mod search;
use search::fold;
pub use search::{TextMatch, find_in_text};
