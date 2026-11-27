use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use yrs::types::Attrs;
use yrs::updates::decoder::Decode;
use yrs::{
    Any, Doc, GetString, OffsetKind, Options, Out, ReadTxn, StateVector, Text, Transact, Update,
};

pub const FIXTURE: &str = include_str!("../../../fixtures/m0/editor-v1.json");

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Snapshot {
    pub version: u32,
    pub revision: u64,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Block {
    pub id: String,
    #[serde(flatten)]
    pub kind: BlockKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockKind {
    Paragraph {
        content: RichText,
    },
    Checklist {
        content: RichText,
        depth: u8,
        checked: bool,
    },
    Table {
        rows: Vec<Vec<RichText>>,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichText {
    pub text: String,
    #[serde(default)]
    pub marks: Vec<Mark>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Mark {
    pub start: u32,
    pub end: u32,
    pub style: Style,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Bold,
    Italic,
}

impl Style {
    fn key(self) -> &'static str {
        match self {
            Self::Bold => "bold",
            Self::Italic => "italic",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Cell {
    pub row: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Operation {
    Replace {
        id: String,
        cell: Option<Cell>,
        start: u32,
        end: u32,
        text: String,
    },
    SplitChecklist {
        id: String,
        offset: u32,
        new_id: String,
    },
    Indent {
        id: String,
        delta: i8,
    },
    Toggle {
        id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Batch {
    pub revision: u64,
    pub operations: Vec<Operation>,
    #[serde(default)]
    pub external: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EditorUpdate {
    pub base_revision: u64,
    pub revision: u64,
    pub patches: Vec<Patch>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Patch {
    Delete { id: String },
    Upsert { index: usize, block: Block },
}

#[derive(Debug, PartialEq, Eq)]
pub enum EditorError {
    InvalidDocument,
    StaleRevision,
    InvalidRange,
    InvalidTarget,
    InvalidOperation,
    HistoryConflict,
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidDocument => "invalid editor document",
            Self::StaleRevision => "editor revision is stale",
            Self::InvalidRange => "range is out of bounds or splits a UTF-16 surrogate pair",
            Self::InvalidTarget => "unknown block or invalid table cell",
            Self::InvalidOperation => "operation is invalid for this block",
            Self::HistoryConflict => "history conflicts with a later update",
        };
        f.write_str(message)
    }
}

impl std::error::Error for EditorError {}

pub fn utf16_to_byte(text: &str, offset: u32) -> Result<usize, EditorError> {
    let mut position = 0;
    for (index, character) in text.char_indices() {
        if position == offset {
            return Ok(index);
        }
        position += character.len_utf16() as u32;
    }
    if position == offset {
        return Ok(text.len());
    }
    Err(EditorError::InvalidRange)
}

pub fn byte_to_utf16(text: &str, offset: usize) -> Result<u32, EditorError> {
    let prefix = text.get(..offset).ok_or(EditorError::InvalidRange)?;
    u32::try_from(prefix.encode_utf16().count()).map_err(|_| EditorError::InvalidRange)
}

struct HistoryEntry {
    forward: Vec<Patch>,
    reverse: Vec<Patch>,
    order: Vec<String>,
}

pub struct Editor {
    snapshot: Snapshot,
    doc: Doc,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
}

impl Editor {
    pub fn open(json: &str) -> Result<Self, EditorError> {
        let snapshot: Snapshot =
            serde_json::from_str(json).map_err(|_| EditorError::InvalidDocument)?;
        let mut ids = HashSet::new();
        if snapshot.version != 1
            || snapshot
                .blocks
                .iter()
                .any(|block| block.id.is_empty() || !ids.insert(&block.id) || !block.is_valid())
        {
            return Err(EditorError::InvalidDocument);
        }
        let doc = text_doc();
        for block in &snapshot.blocks {
            block.write_text(&doc);
        }
        Ok(Self {
            snapshot,
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub fn crdt_state(&self) -> Vec<u8> {
        self.doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default())
    }

    pub fn apply(&mut self, batch: Batch) -> Result<EditorUpdate, EditorError> {
        self.check_revision(batch.revision)?;
        if batch.operations.is_empty() {
            return Ok(self.unchanged());
        }
        let (mut staged, doc) = self.stage()?;
        for operation in batch.operations {
            apply_op(&mut staged.blocks, &doc, operation)?;
        }
        let patches = diff(&self.snapshot.blocks, &staged.blocks);
        let reverse = diff(&staged.blocks, &self.snapshot.blocks);
        // Splits only insert blocks. Undo removes them, and redo uses the
        // original predecessor order to reinsert them after unrelated edits.
        let order = if !batch.external && reverse.iter().any(|p| matches!(p, Patch::Delete { .. }))
        {
            staged.blocks.iter().map(|block| block.id.clone()).collect()
        } else {
            Vec::new()
        };
        let update = self.publish(staged, doc, patches.clone())?;
        if !batch.external && !patches.is_empty() {
            self.undo.push(HistoryEntry {
                forward: patches,
                reverse,
                order,
            });
            self.redo.clear();
        }
        Ok(update)
    }

    pub fn history(&mut self, revision: u64, redo: bool) -> Result<EditorUpdate, EditorError> {
        self.check_revision(revision)?;
        let stack = if redo { &self.redo } else { &self.undo };
        let Some(entry) = stack.last() else {
            return Ok(self.unchanged());
        };
        let (expected, patches) = if redo {
            (&entry.reverse, &entry.forward)
        } else {
            (&entry.forward, &entry.reverse)
        };
        let (mut staged, doc) = self.stage()?;
        for patch in expected {
            match patch {
                Patch::Delete { id } if !staged.blocks.iter().any(|block| block.id == *id) => (),
                Patch::Upsert { block, .. } if staged.blocks.contains(block) => (),
                _ => return Err(EditorError::HistoryConflict),
            }
        }
        for patch in patches {
            match patch {
                Patch::Delete { id } => staged.blocks.retain(|block| block.id != *id),
                Patch::Upsert { index, block } => {
                    block.write_text(&doc);
                    if let Some(existing) =
                        staged.blocks.iter_mut().find(|item| item.id == block.id)
                    {
                        *existing = block.clone();
                    } else {
                        let index = entry
                            .order
                            .get(..*index)
                            .and_then(|predecessors| {
                                predecessors.iter().rev().find_map(|id| {
                                    staged.blocks.iter().position(|old| &old.id == id)
                                })
                            })
                            .map_or(0, |previous| previous + 1);
                        staged.blocks.insert(index, block.clone());
                    }
                }
            }
        }
        let patches = diff(&self.snapshot.blocks, &staged.blocks);
        let update = self.publish(staged, doc, patches)?;
        if redo {
            self.undo.push(self.redo.pop().expect("redo entry exists"));
        } else {
            self.redo.push(self.undo.pop().expect("undo entry exists"));
        }
        Ok(update)
    }

    fn check_revision(&self, revision: u64) -> Result<(), EditorError> {
        if revision != self.snapshot.revision {
            return Err(EditorError::StaleRevision);
        }
        Ok(())
    }

    fn unchanged(&self) -> EditorUpdate {
        EditorUpdate {
            base_revision: self.snapshot.revision,
            revision: self.snapshot.revision,
            patches: Vec::new(),
        }
    }

    fn stage(&self) -> Result<(Snapshot, Doc), EditorError> {
        let doc = text_doc();
        let update =
            Update::decode_v1(&self.crdt_state()).map_err(|_| EditorError::InvalidDocument)?;
        doc.transact_mut()
            .apply_update(update)
            .map_err(|_| EditorError::InvalidDocument)?;
        Ok((self.snapshot.clone(), doc))
    }

    fn publish(
        &mut self,
        mut staged: Snapshot,
        doc: Doc,
        patches: Vec<Patch>,
    ) -> Result<EditorUpdate, EditorError> {
        let base_revision = self.snapshot.revision;
        if patches.is_empty() {
            return Ok(self.unchanged());
        }
        staged.revision = base_revision
            .checked_add(1)
            .ok_or(EditorError::InvalidOperation)?;
        let revision = staged.revision;
        self.snapshot = staged;
        self.doc = doc;
        Ok(EditorUpdate {
            base_revision,
            revision,
            patches,
        })
    }
}

fn text_doc() -> Doc {
    Doc::with_options(Options {
        offset_kind: OffsetKind::Utf16,
        ..Options::default()
    })
}

impl Block {
    fn is_valid(&self) -> bool {
        match &self.kind {
            BlockKind::Paragraph { content } => content.is_valid(),
            BlockKind::Checklist { content, depth, .. } => *depth <= 6 && content.is_valid(),
            BlockKind::Table { rows } => rows.first().is_some_and(|first| {
                !first.is_empty()
                    && rows
                        .iter()
                        .all(|row| row.len() == first.len() && row.iter().all(RichText::is_valid))
            }),
        }
    }

    fn write_text(&self, doc: &Doc) {
        match &self.kind {
            BlockKind::Paragraph { content } | BlockKind::Checklist { content, .. } => {
                content.write(doc, &text_key(&self.id, None));
            }
            BlockKind::Table { rows } => {
                for (row, cells) in rows.iter().enumerate() {
                    for (column, content) in cells.iter().enumerate() {
                        content.write(doc, &text_key(&self.id, Some(Cell { row, column })));
                    }
                }
            }
        }
    }

    fn content_mut(&mut self, cell: Option<Cell>) -> Result<&mut RichText, EditorError> {
        match (&mut self.kind, cell) {
            (BlockKind::Paragraph { content } | BlockKind::Checklist { content, .. }, None) => {
                Ok(content)
            }
            (BlockKind::Table { rows }, Some(Cell { row, column })) => rows
                .get_mut(row)
                .and_then(|cells| cells.get_mut(column))
                .ok_or(EditorError::InvalidTarget),
            _ => Err(EditorError::InvalidTarget),
        }
    }
}

impl RichText {
    fn is_valid(&self) -> bool {
        self.marks.iter().all(|mark| {
            mark.start < mark.end
                && utf16_to_byte(&self.text, mark.start).is_ok()
                && utf16_to_byte(&self.text, mark.end).is_ok()
        })
    }

    fn write(&self, doc: &Doc, key: &str) {
        let target = doc.get_or_insert_text(key);
        let mut txn = doc.transact_mut();
        let len = target.len(&txn);
        if len != 0 {
            target.remove_range(&mut txn, 0, len);
        }
        if !self.text.is_empty() {
            target.insert(&mut txn, 0, &self.text);
        }
        for mark in &self.marks {
            target.format(
                &mut txn,
                mark.start,
                mark.end - mark.start,
                attributes([mark.style]),
            );
        }
    }
}

fn attributes(styles: impl IntoIterator<Item = Style>) -> Attrs {
    styles
        .into_iter()
        .map(|style| (style.key().into(), Any::Bool(true)))
        .collect()
}

fn text_key(id: &str, cell: Option<Cell>) -> String {
    match cell {
        Some(Cell { row, column }) => format!("{row}:{column}:{id}"),
        None => format!("block:{id}"),
    }
}

fn project(doc: &Doc, key: &str) -> RichText {
    let target = doc.get_or_insert_text(key);
    let txn = doc.transact();
    let mut marks: Vec<Mark> = Vec::new();
    let mut position = 0;
    for run in target.diff(&txn, yrs::types::text::YChange::identity) {
        let Out::Any(Any::String(text)) = run.insert else {
            continue;
        };
        let end = position + text.encode_utf16().count() as u32;
        if let Some(attrs) = run.attributes {
            for style in [Style::Bold, Style::Italic] {
                if attrs.get(style.key()) != Some(&Any::Bool(true)) {
                    continue;
                }
                if let Some(last) = marks
                    .iter_mut()
                    .rev()
                    .find(|mark| mark.style == style && mark.end == position)
                {
                    last.end = end;
                } else {
                    marks.push(Mark {
                        start: position,
                        end,
                        style,
                    });
                }
            }
        }
        position = end;
    }
    marks.sort_by_key(|mark| (mark.start, mark.end, mark.style));
    RichText {
        text: target.get_string(&txn),
        marks,
    }
}

fn apply_op(blocks: &mut Vec<Block>, doc: &Doc, operation: Operation) -> Result<(), EditorError> {
    let id = match &operation {
        Operation::Replace { id, .. }
        | Operation::SplitChecklist { id, .. }
        | Operation::Indent { id, .. }
        | Operation::Toggle { id } => id,
    };
    let index = blocks
        .iter()
        .position(|block| &block.id == id)
        .ok_or(EditorError::InvalidTarget)?;
    match operation {
        Operation::Replace {
            id,
            cell,
            start,
            end,
            text,
        } => {
            let content = blocks[index].content_mut(cell)?;
            if start > end {
                return Err(EditorError::InvalidRange);
            }
            utf16_to_byte(&content.text, start)?;
            utf16_to_byte(&content.text, end)?;
            let key = text_key(&id, cell);
            let target = doc.get_or_insert_text(key.as_str());
            let mut txn = doc.transact_mut();
            if start != end {
                target.remove_range(&mut txn, start, end - start);
            }
            if !text.is_empty() {
                let styles = content
                    .marks
                    .iter()
                    .filter(|mark| mark.start <= start && start < mark.end)
                    .map(|mark| mark.style);
                target.insert_with_attributes(&mut txn, start, &text, attributes(styles));
            }
            drop(txn);
            *content = project(doc, &key);
        }
        Operation::SplitChecklist { id, offset, new_id } => {
            if new_id.is_empty() || blocks.iter().any(|block| block.id == new_id) {
                return Err(EditorError::InvalidOperation);
            }
            let BlockKind::Checklist { content, depth, .. } = &mut blocks[index].kind else {
                return Err(EditorError::InvalidOperation);
            };
            let split = utf16_to_byte(&content.text, offset)?;
            let tail = RichText {
                text: content.text.split_off(split),
                marks: content
                    .marks
                    .iter()
                    .filter(|mark| mark.end > offset)
                    .map(|mark| Mark {
                        start: mark.start.saturating_sub(offset),
                        end: mark.end - offset,
                        style: mark.style,
                    })
                    .collect(),
            };
            content.marks.retain_mut(|mark| {
                mark.end = mark.end.min(offset);
                mark.start < mark.end
            });
            let target = doc.get_or_insert_text(text_key(&id, None));
            let mut txn = doc.transact_mut();
            let count = target.len(&txn) - offset;
            if count != 0 {
                target.remove_range(&mut txn, offset, count);
            }
            drop(txn);
            let next = Block {
                id: new_id,
                kind: BlockKind::Checklist {
                    content: tail,
                    depth: *depth,
                    checked: false,
                },
            };
            next.write_text(doc);
            blocks.insert(index + 1, next);
        }
        Operation::Indent { delta, .. } => {
            let BlockKind::Checklist { depth, .. } = &mut blocks[index].kind else {
                return Err(EditorError::InvalidOperation);
            };
            if !matches!(delta, -1 | 1) {
                return Err(EditorError::InvalidOperation);
            }
            *depth = depth.saturating_add_signed(delta).min(6);
        }
        Operation::Toggle { .. } => {
            let BlockKind::Checklist { checked, .. } = &mut blocks[index].kind else {
                return Err(EditorError::InvalidOperation);
            };
            *checked = !*checked;
        }
    }
    Ok(())
}

fn diff(before: &[Block], after: &[Block]) -> Vec<Patch> {
    before
        .iter()
        .filter(|old| !after.iter().any(|block| block.id == old.id))
        .map(|block| Patch::Delete {
            id: block.id.clone(),
        })
        .chain(
            after
                .iter()
                .enumerate()
                .filter(|(_, block)| !before.contains(block))
                .map(|(index, block)| Patch::Upsert {
                    index,
                    block: block.clone(),
                }),
        )
        .collect()
}
