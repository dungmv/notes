use notes_app::editor::{Editor, EditorError as CoreError, FIXTURE};
use serde::Serialize;
use std::sync::Mutex;

#[derive(Debug, uniffi::Error)]
pub enum PrototypeError {
    InvalidDocument,
    StaleRevision,
    InvalidRange,
    InvalidTarget,
    InvalidOperation,
    HistoryConflict,
    InvalidRequest,
    Internal,
}

impl std::fmt::Display for PrototypeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PrototypeError {}

impl From<CoreError> for PrototypeError {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::InvalidDocument => Self::InvalidDocument,
            CoreError::StaleRevision => Self::StaleRevision,
            CoreError::InvalidRange => Self::InvalidRange,
            CoreError::InvalidTarget => Self::InvalidTarget,
            CoreError::InvalidOperation => Self::InvalidOperation,
            CoreError::HistoryConflict => Self::HistoryConflict,
        }
    }
}

#[derive(uniffi::Object)]
pub struct PrototypeEditor {
    editor: Mutex<Editor>,
}

#[uniffi::export]
impl PrototypeEditor {
    #[uniffi::constructor]
    pub fn open_fixture() -> Result<Self, PrototypeError> {
        Self::open_document(FIXTURE.to_owned())
    }

    #[uniffi::constructor]
    pub fn open_document(document: String) -> Result<Self, PrototypeError> {
        Ok(Self {
            editor: Mutex::new(Editor::open(&document)?),
        })
    }

    pub fn snapshot(&self) -> Result<String, PrototypeError> {
        let editor = self.editor.lock().map_err(|_| PrototypeError::Internal)?;
        encode(editor.snapshot())
    }

    pub fn apply(&self, request: String) -> Result<String, PrototypeError> {
        let batch = serde_json::from_str(&request).map_err(|_| PrototypeError::InvalidRequest)?;
        let mut editor = self.editor.lock().map_err(|_| PrototypeError::Internal)?;
        encode(&editor.apply(batch)?)
    }

    pub fn history(&self, revision: u64, redo: bool) -> Result<String, PrototypeError> {
        let mut editor = self.editor.lock().map_err(|_| PrototypeError::Internal)?;
        encode(&editor.history(revision, redo)?)
    }
}

fn encode(value: &impl Serialize) -> Result<String, PrototypeError> {
    serde_json::to_string(value).map_err(|_| PrototypeError::Internal)
}
