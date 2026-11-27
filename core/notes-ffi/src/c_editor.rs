// The Windows prototype owns each handle and serializes calls on its UI thread.
use crate::editor::{PrototypeEditor, PrototypeError};
use serde_json::json;
use std::ffi::{CStr, CString, c_char};

#[unsafe(no_mangle)]
pub extern "C" fn notes_m0_editor_open_fixture() -> *mut PrototypeEditor {
    PrototypeEditor::open_fixture()
        .map(|editor| Box::into_raw(Box::new(editor)))
        .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// A non-null handle must be live for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn notes_m0_editor_snapshot(handle: *const PrototypeEditor) -> *mut c_char {
    // SAFETY: the caller keeps the handle alive.
    reply(
        unsafe { handle.as_ref() }
            .ok_or(PrototypeError::InvalidRequest)
            .and_then(PrototypeEditor::snapshot),
    )
}

/// # Safety
/// A non-null handle must be live. A non-null request must be a readable C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn notes_m0_editor_apply(
    handle: *const PrototypeEditor,
    request: *const c_char,
) -> *mut c_char {
    // SAFETY: the caller keeps the handle and null-terminated request alive.
    reply(
        unsafe { handle.as_ref() }
            .ok_or(PrototypeError::InvalidRequest)
            .and_then(|editor| {
                if request.is_null() {
                    return Err(PrototypeError::InvalidRequest);
                }
                let request = unsafe { CStr::from_ptr(request) }
                    .to_str()
                    .map_err(|_| PrototypeError::InvalidRequest)?;
                editor.apply(request.to_owned())
            }),
    )
}

/// # Safety
/// A non-null handle must be live for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn notes_m0_editor_history(
    handle: *const PrototypeEditor,
    revision: u64,
    redo: u8,
) -> *mut c_char {
    // SAFETY: the caller keeps the handle alive.
    reply(
        unsafe { handle.as_ref() }
            .ok_or(PrototypeError::InvalidRequest)
            .and_then(|editor| editor.history(revision, redo != 0)),
    )
}

/// # Safety
/// A non-null handle must have been returned by the open function and never freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn notes_m0_editor_free(handle: *mut PrototypeEditor) {
    if !handle.is_null() {
        // SAFETY: the caller hands ownership of the handle back exactly once.
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// # Safety
/// A non-null buffer must have been returned by an editor call and never freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn notes_m0_string_free(buffer: *mut c_char) {
    if !buffer.is_null() {
        // SAFETY: the caller hands ownership of the returned string back exactly once.
        drop(unsafe { CString::from_raw(buffer) });
    }
}

fn reply(result: Result<String, PrototypeError>) -> *mut c_char {
    let encoded = match result {
        Ok(value) => json!({ "value": value }),
        Err(error) => json!({ "error": error.to_string() }),
    };
    CString::new(encoded.to_string())
        .expect("JSON escapes null bytes")
        .into_raw()
}
