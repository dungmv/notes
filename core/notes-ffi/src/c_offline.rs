use base64::Engine as _;
use notes_app::editor::{Batch, EditorError};
use notes_app::offline::{NoteEditor, OfflineError, Store, find_in_text};
use notes_app::sync::{SyncClient, SyncError};
use notes_sync::SyncTransportError;
use serde::Deserialize;
use serde_json::{Value, json, to_value};
use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::ptr;
use uuid::Uuid;

#[cfg(test)]
mod tests;

pub struct OfflineSession {
    store: Store,
    note: Option<NoteEditor>,
    sync: SyncClient,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    Folders,
    ChecklistSort,
    SetChecklistSort {
        enabled: bool,
    },
    CreateFolder {
        name: String,
        #[serde(default)]
        parent: Option<String>,
    },
    CreateSmartFolder {
        name: String,
        query: String,
    },
    SmartFolderNotes {
        id: String,
    },
    TagsList,
    ExportNote {
        id: String,
        format: String,
        dir: String,
    },
    ImportFolder {
        path: String,
        parent: Option<String>,
        format: String,
    },
    RenameFolder {
        id: String,
        name: String,
    },
    Notes {
        folder: String,
    },
    Find {
        query: String,
    },
    FindInText {
        text: String,
        query: String,
    },
    Note {
        id: String,
    },
    CreateNote {
        folder: String,
    },
    MoveNote {
        id: String,
        folder: String,
    },
    SetDeleted {
        id: String,
        deleted: bool,
    },
    SetPinned {
        id: String,
        pinned: bool,
    },
    OpenNote {
        id: String,
    },
    Apply {
        batch: Batch,
    },
    History {
        revision: u64,
        redo: bool,
    },
    PasteImage {
        revision: u64,
        id: String,
        start: u32,
        end: u32,
        png: String,
    },
    ImagePath {
        id: String,
    },
    LockStatus,
    LockHint {
        id: String,
    },
    LockNote {
        id: String,
        passphrase: String,
        hint: Option<String>,
    },
    UnlockNote {
        id: String,
        passphrase: String,
    },
    RelockNote {
        id: String,
    },
    LockChangePassword {
        id: String,
        current: String,
        new: String,
        hint: Option<String>,
    },
    LockRemoveProtection {
        id: String,
        passphrase: String,
    },
    ImageData {
        id: String,
    },
    SyncStatus,
    SyncUnlock {
        amk_b64: String,
        refresh_token: String,
    },
    SyncClear,
    SyncRegister {
        server_url: String,
        email: String,
        passphrase: String,
        device_name: String,
        platform: String,
    },
    SyncLogin {
        server_url: String,
        email: String,
        passphrase: String,
        device_name: String,
        platform: String,
    },
    SyncChangePassword {
        current_passphrase: String,
        new_passphrase: String,
    },
    SyncRecover {
        server_url: String,
        email: String,
        recovery_key: String,
        new_passphrase: String,
        device_name: String,
        platform: String,
    },
    SyncListDevices,
    SyncRevokeDevice {
        target_device_id: String,
    },
    SyncNow,
}

type Reply = Result<Value, Box<dyn std::error::Error>>;

impl OfflineSession {
    fn call(&mut self, request: Request) -> Reply {
        let store = &mut self.store;
        let sync = &mut self.sync;
        let result = match request {
            Request::Folders => to_value(store.folders()?),
            Request::ChecklistSort => to_value(store.checklist_sort()?),
            Request::SetChecklistSort { enabled } => {
                store.set_checklist_sort(enabled)?;
                Ok(Value::Null)
            }
            Request::CreateFolder { name, parent } => {
                to_value(store.create_folder(&name, parent.as_deref())?)
            }
            Request::CreateSmartFolder { name, query } => {
                to_value(store.create_smart_folder(&name, &query)?)
            }
            Request::SmartFolderNotes { id } => to_value(store.smart_folder_notes(&id)?),
            Request::TagsList => to_value(store.tags()?),
            Request::ExportNote { id, format, dir } => {
                let path = match format.as_str() {
                    "markdown" => store.export_markdown(&id, Path::new(&dir))?,
                    "html" => store.export_html(&id, Path::new(&dir))?,
                    "pdf" => store.export_pdf(&id, Path::new(&dir))?,
                    _ => return Err(OfflineError::Invalid.into()),
                };
                to_value(path.to_string_lossy())
            }
            Request::ImportFolder { path, parent, format } => {
                let ids = match format.as_str() {
                    "markdown" => {
                        notes_app::transfer::import_markdown_folder(store, Path::new(&path), parent.as_deref())?
                    }
                    "apple_html" => {
                        notes_app::transfer::import_apple_notes_html(store, Path::new(&path), parent.as_deref())?
                    }
                    _ => return Err(OfflineError::Invalid.into()),
                };
                to_value(ids)
            }
            Request::RenameFolder { id, name } => {
                store.rename_folder(&id, &name)?;
                Ok(Value::Null)
            }
            Request::Notes { folder } => to_value(store.notes(&folder)?),
            Request::Find { query } => to_value(store.find(&query)?),
            Request::FindInText { text, query } => to_value(find_in_text(&text, &query)),
            Request::Note { id } => to_value(store.note(&id)?),
            Request::CreateNote { folder } => to_value(store.create_note(&folder)?),
            Request::MoveNote { id, folder } => {
                store.move_note(&id, &folder)?;
                Ok(Value::Null)
            }
            Request::SetDeleted { id, deleted } => {
                store.set_deleted(&id, deleted)?;
                Ok(Value::Null)
            }
            Request::SetPinned { id, pinned } => {
                store.set_pinned(&id, pinned)?;
                Ok(Value::Null)
            }
            Request::OpenNote { id } => {
                let editor = store.open_note(&id)?;
                let result = to_value(editor.snapshot())?;
                self.note = Some(editor);
                Ok(result)
            }
            Request::Apply { batch } => to_value(store.apply(active(&mut self.note)?, batch)?),
            Request::History { revision, redo } => {
                to_value(store.history(active(&mut self.note)?, revision, redo)?)
            }
            Request::PasteImage {
                revision,
                id,
                start,
                end,
                png,
            } => {
                if png.len() > 45_000_000 {
                    return Err(OfflineError::Invalid.into());
                }
                let png = base64::engine::general_purpose::STANDARD.decode(png)?;
                to_value(store.paste_image(
                    active(&mut self.note)?,
                    revision,
                    id,
                    start,
                    end,
                    &png,
                )?)
            }
            Request::ImagePath { id } => to_value(store.image_path(active(&mut self.note)?, &id)?),
            Request::LockStatus => to_value(lock_status(self)?),
            Request::LockHint { id } => to_value(store.lock_hint(&id)?),
            Request::LockNote { id, passphrase, hint } => {
                store.lock_note(&id, &passphrase, hint)?;
                Ok(Value::Null)
            }
            Request::UnlockNote { id, passphrase } => {
                let editor = store.unlock_note(&id, &passphrase)?;
                let result = to_value(editor.snapshot())?;
                self.note = Some(editor);
                Ok(result)
            }
            Request::RelockNote { id } => {
                if let Some(note) = &self.note
                    && note.id() == &id
                    && note.is_locked()
                {
                    self.note = None;
                }
                Ok(Value::Null)
            }
            Request::LockChangePassword { id, current, new, hint } => {
                store.change_lock_password(&id, &current, &new, hint)?;
                Ok(Value::Null)
            }
            Request::LockRemoveProtection { id, passphrase } => {
                store.remove_protection(&id, &passphrase)?;
                if let Some(note) = &self.note
                    && note.id() == &id
                {
                    self.note = None;
                }
                Ok(Value::Null)
            }
            Request::ImageData { id } => {
                let note = active(&mut self.note)?;
                let bytes = store.image_bytes(note, &id)?;
                to_value(json!({
                    "data": base64::engine::general_purpose::STANDARD.encode(bytes),
                    "mime_type": "image/png",
                }))
            }
            Request::SyncStatus => to_value(sync.status(store)?),
            Request::SyncUnlock {
                amk_b64,
                refresh_token,
            } => to_value(sync.unlock_saved_session(store, &amk_b64, &refresh_token)?),
            Request::SyncClear => to_value(sync.clear_session(store)?),
            Request::SyncRegister {
                server_url,
                email,
                passphrase,
                device_name,
                platform,
            } => to_value(sync.register(
                store,
                &server_url,
                &email,
                &passphrase,
                &device_name,
                &platform,
            )?),
            Request::SyncLogin {
                server_url,
                email,
                passphrase,
                device_name,
                platform,
            } => to_value(sync.login(
                store,
                &server_url,
                &email,
                &passphrase,
                &device_name,
                &platform,
            )?),
            Request::SyncChangePassword {
                current_passphrase,
                new_passphrase,
            } => {
                sync.change_password(store, &current_passphrase, &new_passphrase)?;
                Ok(Value::Null)
            }
            Request::SyncRecover {
                server_url,
                email,
                recovery_key,
                new_passphrase,
                device_name,
                platform,
            } => to_value(sync.recover_account(
                store,
                &server_url,
                &email,
                &recovery_key,
                &new_passphrase,
                &device_name,
                &platform,
            )?),
            Request::SyncListDevices => to_value(sync.list_devices(store)?),
            Request::SyncRevokeDevice { target_device_id } => {
                let target = Uuid::parse_str(&target_device_id)?;
                sync.revoke_device(store, target)?;
                Ok(Value::Null)
            }
            Request::SyncNow => {
                let res = sync.sync_now(store)?;
                if let Some(open_id) = self.note.as_ref().map(|n| n.id().to_owned())
                    && res.changed_note_ids.iter().any(|id| id == &open_id)
                {
                    self.note = Some(store.open_note(&open_id)?);
                }
                to_value(res)
            }
        };
        Ok(result?)
    }
}

fn active(note: &mut Option<NoteEditor>) -> Result<&mut NoteEditor, OfflineError> {
    note.as_mut().ok_or(OfflineError::Missing)
}

fn lock_status(session: &OfflineSession) -> Result<Value, OfflineError> {
    let supported = session.store.lock_support();
    let locked_note_ids = if supported {
        session.store.locked_note_ids()?
    } else {
        Vec::new()
    };
    Ok(json!({
        "supported": supported,
        "locked_note_ids": locked_note_ids,
        "open_note_locked": session
            .note
            .as_ref()
            .is_some_and(|note| note.is_locked()),
    }))
}

/// `db_key` là khóa DB cục bộ dạng hex 64 ký tự do client sinh và lưu trong
/// OS secure storage; null hoặc rỗng mở kho dạng rõ (hành vi cũ).
fn parse_db_key(db_key: *const c_char) -> Result<Option<[u8; 32]>, OfflineError> {
    if db_key.is_null() {
        return Ok(None);
    }
    let text = unsafe { CStr::from_ptr(db_key) }
        .to_str()
        .map_err(|_| OfflineError::InvalidDbKey)?;
    if text.is_empty() {
        return Ok(None);
    }
    if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(OfflineError::InvalidDbKey);
    }
    let mut key = [0u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| OfflineError::InvalidDbKey)?;
    }
    Ok(Some(key))
}

fn reply(result: Reply) -> *mut c_char {
    let value = match result {
        Ok(value) => json!({ "value": value }),
        Err(error) => {
            let message = error.to_string();
            if matches!(
                error.downcast_ref::<OfflineError>(),
                Some(OfflineError::Editor(EditorError::HistoryConflict))
            ) {
                json!({ "error": message, "code": "history_conflict" })
            } else if matches!(
                error.downcast_ref::<OfflineError>(),
                Some(OfflineError::NoteLocked)
            ) {
                json!({ "error": message, "code": "note_locked" })
            } else if matches!(
                error.downcast_ref::<OfflineError>(),
                Some(OfflineError::WrongPassphrase)
            ) {
                json!({ "error": message, "code": "wrong_passphrase" })
            } else if matches!(
                error.downcast_ref::<OfflineError>(),
                Some(OfflineError::InvalidDbKey)
            ) {
                json!({ "error": message, "code": "invalid_db_key" })
            } else if matches!(
                error.downcast_ref::<OfflineError>(),
                Some(OfflineError::LockUnsupported)
            ) {
                json!({ "error": message, "code": "lock_unsupported" })
            } else if matches!(
                error.downcast_ref::<OfflineError>(),
                Some(OfflineError::SyncStillEnabled)
            ) {
                json!({ "error": message, "code": "lock_sync_enabled" })
            } else if matches!(
                error.downcast_ref::<SyncError>(),
                Some(SyncError::LockedNotesPresent)
            ) {
                json!({ "error": message, "code": "sync_locked_notes" })
            } else if matches!(
                error.downcast_ref::<SyncError>(),
                Some(
                    SyncError::NotAuthenticated
                        | SyncError::Transport(SyncTransportError::Unauthorized)
                )
            ) {
                json!({ "error": message, "code": "sync_unauthorized" })
            } else if matches!(
                error.downcast_ref::<SyncError>(),
                Some(SyncError::Transport(SyncTransportError::Network(_)))
            ) {
                json!({ "error": message, "code": "sync_network" })
            } else {
                json!({ "error": message })
            }
        }
    };
    CString::new(value.to_string())
        .expect("JSON escapes null bytes")
        .into_raw()
}

/// # Safety
/// Path and `db_key` must be readable UTF-8 C strings; `db_key` may be null
/// to open a plaintext store. Output must accept a session pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn notes_offline_open(
    path: *const c_char,
    db_key: *const c_char,
    output: *mut *mut OfflineSession,
) -> *mut c_char {
    reply((|| {
        if path.is_null() || output.is_null() {
            return Err(OfflineError::Invalid.into());
        }
        unsafe { output.write(ptr::null_mut()) };
        let path = unsafe { CStr::from_ptr(path) }.to_str()?;
        let db_key = parse_db_key(db_key)?;
        let store = Store::open_with_key(Path::new(path), db_key.as_ref())?;
        let session = Box::new(OfflineSession {
            store,
            note: None,
            sync: SyncClient::new(),
        });
        unsafe { output.write(Box::into_raw(session)) };
        Ok(Value::Null)
    })())
}

/// # Safety
/// Handle must be live and exclusively borrowed. Request must be a readable UTF-8 C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn notes_offline_call(
    handle: *mut OfflineSession,
    request: *const c_char,
) -> *mut c_char {
    reply((|| {
        let session = unsafe { handle.as_mut() }.ok_or(OfflineError::Invalid)?;
        if request.is_null() {
            return Err(OfflineError::Invalid.into());
        }
        let request = unsafe { CStr::from_ptr(request) };
        session.call(serde_json::from_slice(request.to_bytes())?)
    })())
}

/// # Safety
/// Handle must be null or have been returned by notes_offline_open and never freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn notes_offline_free(handle: *mut OfflineSession) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}
