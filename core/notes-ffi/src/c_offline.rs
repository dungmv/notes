use base64::Engine as _;
use notes_app::editor::Batch;
use notes_app::offline::{NoteEditor, OfflineError, Store, find_in_text};
use serde::Deserialize;
use serde_json::{Value, json, to_value};
use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::ptr;

#[cfg(test)]
mod tests;

pub struct OfflineSession {
    store: Store,
    note: Option<NoteEditor>,
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
}

type Reply = Result<Value, Box<dyn std::error::Error>>;

impl OfflineSession {
    fn call(&mut self, request: Request) -> Reply {
        let store = &mut self.store;
        let result = match request {
            Request::Folders => to_value(store.folders()?),
            Request::ChecklistSort => to_value(store.checklist_sort()?),
            Request::SetChecklistSort { enabled } => {
                store.set_checklist_sort(enabled)?;
                Ok(Value::Null)
            }
            Request::CreateFolder { name } => to_value(store.create_folder(&name)?),
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
        };
        Ok(result?)
    }
}

fn active(note: &mut Option<NoteEditor>) -> Result<&mut NoteEditor, OfflineError> {
    note.as_mut().ok_or(OfflineError::Missing)
}

fn reply(result: Reply) -> *mut c_char {
    let value = match result {
        Ok(value) => json!({ "value": value }),
        Err(error) => json!({ "error": error.to_string() }),
    };
    CString::new(value.to_string())
        .expect("JSON escapes null bytes")
        .into_raw()
}

/// # Safety
/// Path must be a readable UTF-8 C string. Output must accept a session pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn notes_offline_open(
    path: *const c_char,
    output: *mut *mut OfflineSession,
) -> *mut c_char {
    reply((|| {
        if path.is_null() || output.is_null() {
            return Err(OfflineError::Invalid.into());
        }
        unsafe { output.write(ptr::null_mut()) };
        let path = unsafe { CStr::from_ptr(path) }.to_str()?;
        let store = Store::open(Path::new(path))?;
        let session = Box::new(OfflineSession { store, note: None });
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
