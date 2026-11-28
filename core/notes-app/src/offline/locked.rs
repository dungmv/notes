//! Locked Notes cục bộ: khóa/mở ghi chú bằng mật khẩu riêng (NLK) trên kho
//! SQLite đã mã hóa SQLCipher. Chính sách dữ liệu dẫn xuất theo
//! [02-data-model-and-storage.md](../../../docs/specs/02-data-model-and-storage.md) §3.4:
//! - Khi khóa: `crdt_state` được thay bằng ciphertext, FTS/tag/snippet/cache
//!   văn bản, cờ suy ra và metadata attachment dạng rõ bị xóa trong cùng
//!   transaction; blob ảnh được mã hóa bằng khóa riêng và manifest chứa
//!   metadata attachment được mã hóa bằng NLK. Tiêu đề, thư mục, ghim và mốc
//!   thời gian vẫn hiển thị được.
//! - Phiên mở khóa chỉ dựng nội dung trong bộ nhớ; dữ liệu trên đĩa vẫn ở
//!   dạng đã khóa. Gỡ bảo vệ là thao tác duy nhất khôi phục bản ghi tìm kiếm
//!   và metadata.
//! - Đồng bộ Locked Notes hoãn sang mốc riêng (`TD-003`): khóa ghi chú bị
//!   chặn khi sync đang bật, và bật sync bị chặn khi thư viện có ghi chú khóa.

use super::{
    OfflineError, Store, changed, migrate, now, reindex, write_blob, NoteEditor,
    summarize_note_snapshot,
};
use crate::editor::{Editor, EditorError};
use notes_crypto::keys::{KdfParams, NoteLockKey};
use notes_crypto::notelock::{
    NoteLockManifest, NoteLockMetadata, LockedAttachment, decrypt_locked_blob,
    encrypt_lock_payload, encrypt_locked_blob, generate_blob_id, generate_image_blob_key,
    generate_note_lock_key, unwrap_image_blob_key, unwrap_note_lock_key, wrap_image_blob_key,
    wrap_note_lock_key, LOCK_CONTEXT_MANIFEST, LOCK_CONTEXT_STATE,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Phiên mở khóa của một ghi chú: NLK và manifest đã giải mã chỉ tồn tại
/// trong bộ nhớ và bị zeroize khi phiên kết thúc.
pub(super) struct LockedSession {
    pub(super) nlk: NoteLockKey,
    pub(super) manifest: NoteLockManifest,
}

impl LockedSession {
    pub(super) fn nlk(&self) -> &NoteLockKey {
        &self.nlk
    }
}

struct AttachmentRow {
    id: String,
    kind: String,
    file_name: String,
    mime_type: String,
    byte_size: i64,
    content_hash: String,
    created_at: i64,
}

fn lock_crypto<T>(result: Result<T, notes_crypto::CryptoError>) -> Result<T, OfflineError> {
    result.map_err(|error| match error {
        notes_crypto::CryptoError::AuthenticationFailed => OfflineError::WrongPassphrase,
        other => OfflineError::Crypto(other),
    })
}

fn lock_kdf_params() -> KdfParams {
    KdfParams::default()
}

fn clean_hint(hint: Option<String>) -> Result<Option<String>, OfflineError> {
    match hint {
        None => Ok(None),
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            if trimmed.chars().count() > 160 {
                return Err(OfflineError::Invalid);
            }
            Ok(Some(trimmed.to_owned()))
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((byte >> 4) as u32, 16).expect("high nibble"));
        out.push(char::from_digit((byte & 0x0f) as u32, 16).expect("low nibble"));
    }
    out
}

fn sql_quote_path(path: &Path) -> Result<String, OfflineError> {
    let text = path
        .to_str()
        .ok_or(OfflineError::Invalid)?
        .replace('\'', "''");
    Ok(text)
}

fn is_plaintext_sqlite_file(path: &Path) -> Result<bool, OfflineError> {
    let mut header = [0u8; 16];
    let mut file = fs::File::open(path)?;
    file.read_exact(&mut header)?;
    Ok(header == *b"SQLite format 3\0")
}

fn open_encrypted_connection(path: &Path, key: &[u8; 32]) -> Result<Connection, OfflineError> {
    let db = Connection::open(path)?;
    // Raw hex key: no SQLCipher KDF on top, key material lives in OS secure storage.
    db.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex_encode(key)))?;
    // Force a read of the database header to validate the key before use.
    let probe: Result<i64, rusqlite::Error> =
        db.query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0));
    if let Err(error) = probe {
        if error.sqlite_error_code() == Some(rusqlite::ErrorCode::NotADatabase) {
            return Err(OfflineError::InvalidDbKey);
        }
        return Err(error.into());
    }
    Ok(db)
}

fn encrypted_pragmas(db: &mut Connection) -> Result<(), OfflineError> {
    db.busy_timeout(Duration::from_secs(3))?;
    let mode: String = db.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(OfflineError::Invalid);
    }
    db.execute_batch("PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")?;
    Ok(())
}

/// Chuyển kho SQLite dạng rõ sang kho SQLCipher cùng schema hiện có. Kho cũ
/// (DB, WAL, SHM) được dọn sau khi bản mã hóa đã dựng và kiểm tra xong.
fn encrypt_database_file(db_path: &Path, key: &[u8; 32]) -> Result<(), OfflineError> {
    let mut plain = Connection::open(db_path)?;
    encrypted_pragmas(&mut plain)?;
    migrate(&mut plain)?;
    plain.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    drop(plain);

    let target = db_path.with_extension("sqlite3.enc");
    let _ = fs::remove_file(&target);
    let mut encrypted = open_encrypted_connection(&target, key)?;
    encrypted_pragmas(&mut encrypted)?;
    migrate(&mut encrypted)?;

    // ATTACH kho dạng rõ phải khai báo KEY rỗng, nếu không SQLCipher sẽ dùng
    // khóa của kho chính để giải mã và đọc sai file.
    encrypted.execute_batch(&format!(
        "ATTACH DATABASE '{}' AS plain KEY '';",
        sql_quote_path(db_path)?
    ))?;
    // Xóa theo thứ tự con -> cha rồi chép ngược lại theo thứ tự cha -> con
    // để không vi phạm khóa ngoại.
    for table in [
        "attachments",
        "notes",
        "folders",
        "notes_fts",
        "preferences",
        "sync_config",
        "sync_outbox",
        "sync_blobs",
        "sync_lww",
    ] {
        encrypted.execute(&format!("DELETE FROM main.{table}"), [])?;
    }
    for table in [
        "folders",
        "notes",
        "attachments",
        "preferences",
        "sync_config",
        "sync_outbox",
        "sync_blobs",
        "sync_lww",
        "notes_fts",
    ] {
        encrypted.execute(&format!("INSERT INTO main.{table} SELECT * FROM plain.{table}"), [])?;
    }
    encrypted.execute_batch("DETACH DATABASE plain;")?;
    drop(encrypted);

    for suffix in ["sqlite3-wal", "sqlite3-shm"] {
        let _ = fs::remove_file(db_path.with_extension(suffix));
    }
    fs::rename(&target, db_path)?;
    Ok(())
}

impl Store {
    /// Mở kho với khóa DB cục bộ. `None` giữ hành vi cũ (kho dạng rõ, dùng
    /// cho kho không bật Locked Notes và các kiểm thử hiện có). Kho dạng rõ
    /// kèm khóa sẽ được chuyển sang SQLCipher khi mở.
    pub fn open_with_key(root: &Path, db_key: Option<&[u8; 32]>) -> Result<Store, OfflineError> {
        let Some(key) = db_key else {
            return Store::open(root);
        };
        fs::create_dir_all(root)?;
        let db_path = root.join("notes.sqlite3");
        if db_path.is_file() && is_plaintext_sqlite_file(&db_path)? {
            encrypt_database_file(&db_path, key)?;
        }
        let mut db = open_encrypted_connection(&db_path, key)?;
        encrypted_pragmas(&mut db)?;
        migrate(&mut db)?;
        Ok(Store {
            db,
            root: root.to_owned(),
            encrypted: true,
        })
    }

    pub fn lock_support(&self) -> bool {
        self.encrypted
    }

    pub fn locked_note_ids(&self) -> Result<Vec<String>, OfflineError> {
        let mut query = self
            .db
            .prepare("SELECT id FROM notes WHERE is_locked=1 ORDER BY updated_at DESC, id")?;
        Ok(query
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    /// Gợi ý mật khẩu khóa hiển thị trên màn hình khóa; không mở nội dung.
    pub fn lock_hint(&self, id: &str) -> Result<Option<String>, OfflineError> {
        let blob: Option<Vec<u8>> = self
            .db
            .query_row(
                "SELECT lock_metadata FROM notes WHERE id=?1 AND is_locked=1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(blob) = blob else {
            return Ok(None);
        };
        let metadata: NoteLockMetadata = serde_json::from_slice(&blob)?;
        Ok(metadata.hint)
    }

    fn sync_enabled(&self) -> Result<bool, OfflineError> {
        Ok(self.db.query_row(
            "SELECT enabled FROM sync_config WHERE id=1",
            [],
            |row| row.get::<_, i64>(0),
        )? != 0)
    }

    fn lockable_attachments(&self, note_id: &str) -> Result<Vec<AttachmentRow>, OfflineError> {
        let mut query = self.db.prepare(
            "SELECT id,kind,file_name,mime_type,byte_size,content_hash,created_at
            FROM attachments WHERE note_id=?1 ORDER BY created_at,id",
        )?;
        Ok(query
            .query_map([note_id], |row| {
                Ok(AttachmentRow {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    file_name: row.get(2)?,
                    mime_type: row.get(3)?,
                    byte_size: row.get(4)?,
                    content_hash: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }

    fn read_lock_row(&self, id: &str) -> Result<(Vec<u8>, Option<Vec<u8>>, Vec<u8>, i64), OfflineError> {
        self.db
            .query_row(
                "SELECT lock_metadata, lock_manifest, crdt_state, revision
                FROM notes WHERE id=?1 AND is_locked=1",
                [id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or(OfflineError::Missing)
    }

    fn decrypt_manifest(
        &self,
        id: &str,
        nlk: &NoteLockKey,
        manifest_blob: Option<Vec<u8>>,
    ) -> Result<NoteLockManifest, OfflineError> {
        match manifest_blob {
            Some(bytes) => {
                let plain = lock_crypto(notes_crypto::notelock::decrypt_lock_payload(
                    nlk.as_bytes(),
                    id,
                    LOCK_CONTEXT_MANIFEST,
                    &bytes,
                ))?;
                Ok(serde_json::from_slice(&plain)?)
            }
            None => Ok(NoteLockManifest {
                version: notes_crypto::notelock::LOCK_MANIFEST_VERSION,
                attachments: Vec::new(),
            }),
        }
    }

    /// Khóa một ghi chú thường bằng mật khẩu khóa. Blob ảnh được mã hóa và
    /// ghi bền vững trước khi commit; plaintext blob được dọn sau commit khi
    /// không còn tham chiếu nào khác.
    pub fn lock_note(
        &mut self,
        id: &str,
        passphrase: &str,
        hint: Option<String>,
    ) -> Result<(), OfflineError> {
        if !self.encrypted {
            return Err(OfflineError::LockUnsupported);
        }
        if passphrase.is_empty() {
            return Err(OfflineError::Invalid);
        }
        let hint = clean_hint(hint)?;
        if self.sync_enabled()? {
            return Err(OfflineError::SyncStillEnabled);
        }
        let (state, locked, deleted): (Vec<u8>, i64, Option<i64>) = self
            .db
            .query_row(
                "SELECT crdt_state, is_locked, deleted_at FROM notes WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(OfflineError::Missing)?;
        if locked != 0 || deleted.is_some() {
            return Err(OfflineError::Invalid);
        }
        let attachments = self.lockable_attachments(id)?;
        let nlk = lock_crypto(generate_note_lock_key())?;
        let metadata = lock_crypto(wrap_note_lock_key(
            id,
            passphrase.as_bytes(),
            &nlk,
            &lock_kdf_params(),
            hint,
            now(),
            now(),
        ))?;

        let mut entries = Vec::with_capacity(attachments.len());
        for attachment in &attachments {
            let plaintext = self.read_blob_bytes(&attachment.content_hash)?;
            let key = lock_crypto(generate_image_blob_key())?;
            let blob_id = generate_blob_id();
            let ciphertext = lock_crypto(encrypt_locked_blob(&blob_id, &key, &plaintext))?;
            let path: PathBuf = self.blob_path(&blob_id)?;
            write_blob(&path, &ciphertext)?;
            entries.push(LockedAttachment {
                id: attachment.id.clone(),
                kind: attachment.kind.clone(),
                file_name: attachment.file_name.clone(),
                mime_type: attachment.mime_type.clone(),
                byte_size: attachment.byte_size,
                content_hash: attachment.content_hash.clone(),
                wrapped_key_hex: lock_crypto(wrap_image_blob_key(id, &nlk, &key))?,
                blob_id,
                created_at: attachment.created_at,
            });
        }
        let manifest = NoteLockManifest {
            version: notes_crypto::notelock::LOCK_MANIFEST_VERSION,
            attachments: entries,
        };
        let manifest_plain = serde_json::to_vec(&manifest)?;
        let manifest_envelope = lock_crypto(encrypt_lock_payload(
            nlk.as_bytes(),
            id,
            LOCK_CONTEXT_MANIFEST,
            &manifest_plain,
        ))?;
        let state_envelope =
            lock_crypto(encrypt_lock_payload(nlk.as_bytes(), id, LOCK_CONTEXT_STATE, &state))?;
        let metadata_blob = serde_json::to_vec(&metadata)?;

        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        changed(tx.execute(
            "UPDATE notes SET is_locked=1, lock_metadata=?1, lock_manifest=?2, crdt_state=?3,
            snippet='', plain_text_cache='', has_checklist=0, has_attachments=0
            WHERE id=?4 AND is_locked=0 AND deleted_at IS NULL",
            params![metadata_blob, manifest_envelope, state_envelope, id],
        )?)?;
        tx.execute("DELETE FROM attachments WHERE note_id=?1", [id])?;
        tx.execute("DELETE FROM note_tags WHERE note_id=?1", [id])?;
        reindex(&tx, id)?;
        tx.execute(
            "DELETE FROM sync_outbox WHERE entity_type='note' AND entity_id=?1 AND acked=0",
            [id],
        )?;
        tx.commit()?;

        // Dọn bản rõ blob sau commit: chỉ xóa khi không còn ghi chú thường nào
        // tham chiếu cùng hash và không còn item outbox chờ gửi giữ hash đó.
        for attachment in &attachments {
            let referenced: bool = self.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM attachments WHERE content_hash=?1)
                OR EXISTS(SELECT 1 FROM sync_outbox WHERE acked=0 AND blob_hashes_json LIKE '%'||?1||'%')",
                [&attachment.content_hash],
                |row| row.get(0),
            )?;
            if !referenced {
                let path = self.blob_path(&attachment.content_hash)?;
                let _ = fs::remove_file(path);
            }
        }
        Ok(())
    }

    /// Mở phiên đọc/sửa nội dung ghi chú khóa. NLK và nội dung dựng sẵn chỉ
    /// tồn tại trong bộ nhớ của phiên; dữ liệu trên đĩa giữ nguyên dạng khóa.
    pub fn unlock_note(
        &mut self,
        id: &str,
        passphrase: &str,
    ) -> Result<NoteEditor, OfflineError> {
        if !self.encrypted {
            return Err(OfflineError::LockUnsupported);
        }
        let (metadata_blob, manifest_blob, state, revision) = self.read_lock_row(id)?;
        let metadata: NoteLockMetadata = serde_json::from_slice(&metadata_blob)?;
        let nlk = lock_crypto(unwrap_note_lock_key(id, passphrase.as_bytes(), &metadata))?;
        let manifest = self.decrypt_manifest(id, &nlk, manifest_blob)?;
        let plain = lock_crypto(notes_crypto::notelock::decrypt_lock_payload(
            nlk.as_bytes(),
            id,
            LOCK_CONTEXT_STATE,
            &state,
        ))?;
        let editor = Editor::from_state(&plain)?;
        if u64::try_from(revision).ok() != Some(editor.snapshot().revision) {
            return Err(EditorError::InvalidDocument.into());
        }
        Ok(NoteEditor {
            id: id.to_owned(),
            editor,
            lock: Some(LockedSession { nlk, manifest }),
        })
    }

    /// Đổi mật khẩu khóa: unwrap NLK bằng mật khẩu cũ và re-wrap với salt/KDF
    /// mới. Nội dung và blob không cần mã hóa lại.
    pub fn change_lock_password(
        &mut self,
        id: &str,
        current: &str,
        new: &str,
        hint: Option<String>,
    ) -> Result<(), OfflineError> {
        if !self.encrypted {
            return Err(OfflineError::LockUnsupported);
        }
        if new.is_empty() {
            return Err(OfflineError::Invalid);
        }
        let hint = clean_hint(hint)?;
        let (metadata_blob, _, _, _) = self.read_lock_row(id)?;
        let metadata: NoteLockMetadata = serde_json::from_slice(&metadata_blob)?;
        let nlk = lock_crypto(unwrap_note_lock_key(id, current.as_bytes(), &metadata))?;
        let hint = hint.or(metadata.hint.clone());
        let updated = lock_crypto(wrap_note_lock_key(
            id,
            new.as_bytes(),
            &nlk,
            &metadata.kdf.clone(),
            hint,
            metadata.created_at,
            now(),
        ))?;
        changed(self.db.execute(
            "UPDATE notes SET lock_metadata=?1 WHERE id=?2 AND is_locked=1",
            params![serde_json::to_vec(&updated)?, id],
        )?)?;
        Ok(())
    }

    /// Gỡ bảo vệ hoàn toàn: khôi phục nội dung dạng rõ, metadata attachment,
    /// bản ghi FTS và các cờ suy ra. Đây là thao tác duy nhất khôi phục dữ
    /// liệu dẫn xuất theo chính sách §3.4.
    pub fn remove_protection(&mut self, id: &str, passphrase: &str) -> Result<(), OfflineError> {
        if !self.encrypted {
            return Err(OfflineError::LockUnsupported);
        }
        let (metadata_blob, manifest_blob, state, revision) = self.read_lock_row(id)?;
        let metadata: NoteLockMetadata = serde_json::from_slice(&metadata_blob)?;
        let nlk = lock_crypto(unwrap_note_lock_key(id, passphrase.as_bytes(), &metadata))?;
        let manifest = self.decrypt_manifest(id, &nlk, manifest_blob)?;
        let plain = lock_crypto(notes_crypto::notelock::decrypt_lock_payload(
            nlk.as_bytes(),
            id,
            LOCK_CONTEXT_STATE,
            &state,
        ))?;
        let editor = Editor::from_state(&plain)?;
        if u64::try_from(revision).ok() != Some(editor.snapshot().revision) {
            return Err(EditorError::InvalidDocument.into());
        }

        // Khôi phục blob dạng rõ trước khi commit, đúng chính sách hoàn tất
        // ghi bền vững file rồi mới commit tham chiếu.
        for entry in &manifest.attachments {
            let key = lock_crypto(unwrap_image_blob_key(id, &nlk, &entry.wrapped_key_hex))?;
            let ciphertext = fs::read(self.blob_path(&entry.blob_id)?)?;
            let plaintext = lock_crypto(decrypt_locked_blob(&entry.blob_id, &key, &ciphertext))?;
            write_blob(&self.blob_path(&entry.content_hash)?, &plaintext)?;
        }

        let (title, snippet, text, checklist, images) = summarize_note_snapshot(editor.snapshot());
        let ts = now();
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        changed(tx.execute(
            "UPDATE notes SET is_locked=0, lock_metadata=NULL, lock_manifest=NULL, crdt_state=?1,
            title=?2, snippet=?3, plain_text_cache=?4, has_checklist=?5, has_attachments=?6,
            updated_at=?7 WHERE id=?8 AND is_locked=1",
            params![plain, title, snippet, text, checklist, images, ts, id],
        )?)?;
        tx.execute("DELETE FROM attachments WHERE note_id=?1", [id])?;
        for entry in &manifest.attachments {
            tx.execute(
                "INSERT INTO attachments(id,note_id,kind,file_name,mime_type,byte_size,content_hash,created_at)
                VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    entry.id,
                    id,
                    entry.kind,
                    entry.file_name,
                    entry.mime_type,
                    entry.byte_size,
                    entry.content_hash,
                    entry.created_at
                ],
            )?;
        }
        reindex(&tx, id)?;
        super::update_tags(&tx, id, &text)?;
        tx.commit()?;

        // Blob mã hóa không còn tham chiếu sau khi gỡ bảo vệ.
        for entry in &manifest.attachments {
            let path = self.blob_path(&entry.blob_id)?;
            let _ = fs::remove_file(path);
        }
        Ok(())
    }

    /// Trả về nội dung blob ảnh để hiển thị: giải mã trực tiếp cho ghi chú
    /// khóa trong phiên mở, đọc thường cho ghi chú thường.
    pub fn image_bytes(
        &self,
        note: &NoteEditor,
        attachment_id: &str,
    ) -> Result<Vec<u8>, OfflineError> {
        if let Some(lock) = &note.lock {
            let entry = lock
                .manifest
                .attachments
                .iter()
                .find(|entry| entry.id == attachment_id)
                .ok_or(OfflineError::Missing)?;
            let key = lock_crypto(unwrap_image_blob_key(&note.id, &lock.nlk, &entry.wrapped_key_hex))?;
            let ciphertext = fs::read(self.blob_path(&entry.blob_id)?)?;
            return lock_crypto(decrypt_locked_blob(&entry.blob_id, &key, &ciphertext));
        }
        let hash: String = self
            .db
            .query_row(
                "SELECT content_hash FROM attachments WHERE id=?1 AND note_id=?2",
                params![attachment_id, note.id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(OfflineError::Missing)?;
        self.read_blob_bytes(&hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{Batch, Operation};

    const PNG: &[u8] = include_bytes!("../../../../fixtures/m1/pixel.png");
    const PASSPHRASE: &str = "mat-khau-khoa";

    fn key_hex(seed: u8) -> [u8; 32] {
        let mut key = [seed; 32];
        key[31] = seed.wrapping_add(1);
        key
    }

    fn key_string(seed: u8) -> String {
        hex_encode(&key_hex(seed))
    }

    fn open_encrypted(root: &Path, seed: u8) -> Store {
        Store::open_with_key(root, Some(&key_hex(seed))).unwrap()
    }

    fn image_attachment_id(editor: &NoteEditor) -> String {
        editor
            .snapshot()
            .blocks
            .iter()
            .find_map(|block| match &block.kind {
                crate::editor::BlockKind::Image { attachment_id } => Some(attachment_id.clone()),
                _ => None,
            })
            .expect("note contains an image")
    }

    fn note_with_image(root: &Path, seed: u8) -> (Store, String) {
        let mut store = open_encrypted(root, seed);
        let id = store.create_note("inbox").unwrap();
        let mut editor = store.open_note(&id).unwrap();
        let block = editor.snapshot().blocks[0].id.clone();
        store
            .apply(
                &mut editor,
                Batch {
                    revision: 0,
                    operations: vec![Operation::Replace {
                        id: block.clone(),
                        cell: None,
                        start: 0,
                        end: 0,
                        text: "Ghi chú có ảnh riêng tư".to_owned(),
                    }],
                    external: false,
                },
            )
            .unwrap();
        store
            .paste_image(&mut editor, 1, block, 7, 7, PNG)
            .unwrap();
        (store, id)
    }

    #[test]
    fn lock_requires_sqlcipher_store() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = Store::open(temp.path()).unwrap();
        let id = store.create_note("inbox").unwrap();
        let error = store.lock_note(&id, PASSPHRASE, None).unwrap_err();
        assert!(matches!(error, OfflineError::LockUnsupported));
    }

    #[test]
    fn lock_scrubs_derived_data_and_cleans_plaintext_blobs() {
        let temp = tempfile::tempdir().unwrap();
        let (mut store, id) = note_with_image(temp.path(), 1);
        let plaintext_blob: PathBuf = {
            let hash: String = store
                .db()
                .query_row(
                    "SELECT content_hash FROM attachments WHERE note_id=?1",
                    [&id],
                    |row| row.get(0),
                )
                .unwrap();
            store.blob_path(&hash).unwrap()
        };
        assert!(plaintext_blob.is_file());

        store.lock_note(&id, PASSPHRASE, None).unwrap();

        let (locked, cached, snippet, checklist, attachments): (i64, String, String, i64, i64) =
            store
                .db()
                .query_row(
                    "SELECT is_locked, plain_text_cache, snippet, has_checklist,
                    (SELECT COUNT(*) FROM attachments WHERE note_id=?1)
                    FROM notes WHERE id=?1",
                    [&id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .unwrap();
        assert_eq!(locked, 1);
        assert_eq!(cached, "");
        assert_eq!(snippet, "");
        assert_eq!(checklist, 0);
        assert_eq!(attachments, 0);
        let fts_rows: i64 = store
            .db()
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE note_id=?1",
                [&id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fts_rows, 0);
        assert!(!plaintext_blob.is_file(), "plaintext blob phải được dọn");
        let title = store.note(&id).unwrap();
        assert_eq!(title.title, "Ghi chú");

        // Tìm theo nội dung không còn trả về, tìm theo tiêu đề thì có.
        assert!(store.find("anh rieng tu").unwrap().is_empty());
        let by_title = store.find("ghi chu").unwrap();
        assert_eq!(by_title.len(), 1);
        assert_eq!(by_title[0].id, id);
        assert!(by_title[0].locked);
    }

    #[test]
    fn unlock_edit_relock_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let (mut store, id) = note_with_image(temp.path(), 2);
        store.lock_note(&id, PASSPHRASE, None).unwrap();

        let mut editor = store.unlock_note(&id, PASSPHRASE).unwrap();
        let block = editor.snapshot().blocks[0].id.clone();
        let update = store
            .apply(
                &mut editor,
                Batch {
                    revision: 2,
                    operations: vec![Operation::Replace {
                        id: block,
                        cell: None,
                        start: 7,
                        end: 7,
                        text: " mới".to_owned(),
                    }],
                    external: false,
                },
            )
            .unwrap();
        assert_eq!(update.revision, 3);

        // Bản lưu trên đĩa vẫn ở dạng khóa trong suốt phiên mở.
        let editor = store.unlock_note(&id, PASSPHRASE).unwrap();
        let text = editor
            .snapshot()
            .blocks
            .iter()
            .map(|block| match &block.kind {
                crate::editor::BlockKind::Paragraph { content } => content.text.clone(),
                _ => String::new(),
            })
            .collect::<String>();
        assert!(text.contains("chú mới"));
        drop(editor);
        drop(store);

        let mut store = open_encrypted(temp.path(), 2);
        assert_eq!(store.locked_note_ids().unwrap(), vec![id.clone()]);
        let editor = store.unlock_note(&id, PASSPHRASE).unwrap();
        let attachment = image_attachment_id(&editor);
        let bytes = store.image_bytes(&editor, &attachment).unwrap();
        assert_eq!(bytes, PNG);
    }

    #[test]
    fn wrong_passphrase_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let (mut store, id) = note_with_image(temp.path(), 3);
        store.lock_note(&id, PASSPHRASE, None).unwrap();
        let error = store.unlock_note(&id, "sai-mat-khau").map(|_| ()).unwrap_err();
        assert!(matches!(error, OfflineError::WrongPassphrase));
    }

    #[test]
    fn remove_protection_restores_metadata_and_blobs() {
        let temp = tempfile::tempdir().unwrap();
        let (mut store, id) = note_with_image(temp.path(), 4);
        store.lock_note(&id, PASSPHRASE, None).unwrap();
        store.remove_protection(&id, PASSPHRASE).unwrap();

        let note = store.note(&id).unwrap();
        assert!(!note.locked);
        let attachments: i64 = store
            .db()
            .query_row(
                "SELECT COUNT(*) FROM attachments WHERE note_id=?1",
                [&id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(attachments, 1);
        let editor = store.open_note(&id).unwrap();
        let attachment = image_attachment_id(&editor);
        assert_eq!(store.image_bytes(&editor, &attachment).unwrap(), PNG);
        assert_eq!(store.find("anh rieng tu").unwrap()[0].id, id);
        // Khóa lại bằng mật khẩu khác sau khi gỡ bảo vệ.
        store.lock_note(&id, "mat-khau-moi", None).unwrap();
        let error = store.unlock_note(&id, PASSPHRASE).map(|_| ()).unwrap_err();
        assert!(matches!(error, OfflineError::WrongPassphrase));
        store.unlock_note(&id, "mat-khau-moi").unwrap();
    }

    #[test]
    fn change_lock_password_rewraps_key() {
        let temp = tempfile::tempdir().unwrap();
        let (mut store, id) = note_with_image(temp.path(), 5);
        store.lock_note(&id, PASSPHRASE, Some("gợi ý".to_owned())).unwrap();
        store.change_lock_password(&id, PASSPHRASE, "mat-khau-moi", None).unwrap();
        let error = store.unlock_note(&id, PASSPHRASE).map(|_| ()).unwrap_err();
        assert!(matches!(error, OfflineError::WrongPassphrase));
        store.unlock_note(&id, "mat-khau-moi").unwrap();
        let metadata: Vec<u8> = store
            .db()
            .query_row(
                "SELECT lock_metadata FROM notes WHERE id=?1",
                [&id],
                |row| row.get(0),
            )
            .unwrap();
        let metadata = String::from_utf8(metadata).unwrap();
        assert!(metadata.contains("gợi ý"), "hint cũ phải được giữ lại");
    }

    #[test]
    fn sync_enabled_blocks_locking() {
        let temp = tempfile::tempdir().unwrap();
        let (mut store, id) = note_with_image(temp.path(), 6);
        store
            .db_mut()
            .execute("UPDATE sync_config SET enabled=1 WHERE id=1", [])
            .unwrap();
        let error = store.lock_note(&id, PASSPHRASE, None).unwrap_err();
        assert!(matches!(error, OfflineError::SyncStillEnabled));
    }

    #[test]
    fn plaintext_store_migrates_and_wrong_key_fails() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let (store, id) = {
            let mut store = Store::open(root).unwrap();
            let id = store.create_note("inbox").unwrap();
            let mut editor = store.open_note(&id).unwrap();
            let block = editor.snapshot().blocks[0].id.clone();
            store
                .apply(
                    &mut editor,
                    Batch {
                        revision: 0,
                        operations: vec![Operation::Replace {
                            id: block,
                            cell: None,
                            start: 0,
                            end: 0,
                            text: "Dữ liệu cũ".to_owned(),
                        }],
                        external: false,
                    },
                )
                .unwrap();
            (store, id)
        };
        let db_path = root.join("notes.sqlite3");
        drop(store);
        assert!(std::fs::read(&db_path).unwrap().starts_with(b"SQLite format 3"));

        let store = open_encrypted(root, 7);
        assert_eq!(store.note(&id).unwrap().title, "Dữ liệu cũ");
        assert!(!std::fs::read(&db_path).unwrap().starts_with(b"SQLite format 3"));
        drop(store);

        let error = Store::open_with_key(root, Some(&key_hex(8)))
            .map(|_| ())
            .unwrap_err();
        assert!(
            matches!(error, OfflineError::InvalidDbKey),
            "sai khóa phải bị từ chối ngay khi mở kho"
        );
    }

    #[test]
    fn locked_note_survives_restart() {
        let temp = tempfile::tempdir().unwrap();
        let (mut store, id) = note_with_image(temp.path(), 9);
        store.lock_note(&id, PASSPHRASE, None).unwrap();
        drop(store);
        let mut store = open_encrypted(temp.path(), 9);
        assert_eq!(store.locked_note_ids().unwrap(), vec![id.clone()]);
        let error = store.open_note(&id).map(|_| ()).unwrap_err();
        assert!(matches!(error, OfflineError::NoteLocked));
        store.unlock_note(&id, PASSPHRASE).unwrap();
    }
}
