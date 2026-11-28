use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use yrs::types::Attrs;
use yrs::updates::decoder::Decode;
use yrs::{
    Any, Doc, GetString, Map, OffsetKind, Options, Out, ReadTxn, StateVector, Text, Transact,
    Update,
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
    Styled {
        content: RichText,
        style: ParagraphStyle,
    },
    Checklist {
        content: RichText,
        depth: u8,
        checked: bool,
    },
    Table {
        rows: Vec<Vec<RichText>>,
    },
    Image {
        attachment_id: String,
    },
    NoteLink {
        note_id: String,
        label: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParagraphStyle {
    Title,
    Heading,
    Subheading,
    Quote,
    Monospaced,
    Bullet,
    Dashed,
    Numbered,
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
    Underline,
    Strikethrough,
}

impl Style {
    fn key(self) -> &'static str {
        match self {
            Self::Bold => "bold",
            Self::Italic => "italic",
            Self::Underline => "underline",
            Self::Strikethrough => "strikethrough",
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
    SplitParagraph {
        id: String,
        offset: u32,
        new_id: String,
    },
    MergePrevious {
        id: String,
    },
    Indent {
        id: String,
        delta: i8,
    },
    Toggle {
        id: String,
    },
    Format {
        id: String,
        cell: Option<Cell>,
        start: u32,
        end: u32,
        style: Style,
    },
    ParagraphStyle {
        id: String,
        style: Option<ParagraphStyle>,
        checklist: bool,
    },
    InsertAt {
        id: String,
        start: u32,
        end: u32,
        block: Block,
        tail_id: String,
    },
    RemoveEmbed {
        id: String,
    },
    TableInsertRow {
        id: String,
        after_row: Option<usize>,
    },
    TableRemoveRow {
        id: String,
        row: usize,
    },
    TableInsertColumn {
        id: String,
        after_column: Option<usize>,
    },
    TableRemoveColumn {
        id: String,
        column: usize,
    },
    TableMoveRow {
        id: String,
        from: usize,
        to: usize,
    },
    TableMoveColumn {
        id: String,
        from: usize,
        to: usize,
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
    order: [Vec<String>; 2],
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
        Self::validate(&snapshot)?;
        let doc = text_doc();
        for block in &snapshot.blocks {
            block.write_text(&doc);
        }
        write_layout(&doc, &snapshot)?;
        Ok(Self::from_parts(snapshot, doc))
    }

    fn validate(snapshot: &Snapshot) -> Result<(), EditorError> {
        let mut ids = HashSet::new();
        if snapshot.version != 1
            || snapshot
                .blocks
                .iter()
                .any(|block| block.id.is_empty() || !ids.insert(&block.id) || !block.is_valid())
        {
            return Err(EditorError::InvalidDocument);
        }
        Ok(())
    }

    fn from_parts(snapshot: Snapshot, doc: Doc) -> Self {
        Self {
            snapshot,
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn from_state(state: &[u8]) -> Result<Self, EditorError> {
        let doc = load_doc(state)?;
        let snapshot = {
            let txn = doc.transact();
            let Some(Out::Any(Any::String(layout))) = txn
                .get_map("document")
                .and_then(|map| map.get(&txn, "layout"))
            else {
                return Err(EditorError::InvalidDocument);
            };
            serde_json::from_str::<Snapshot>(&layout).map_err(|_| EditorError::InvalidDocument)?
        };
        Self::validate(&snapshot)?;
        Ok(Self::from_parts(snapshot, doc))
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub fn crdt_state(&self) -> Vec<u8> {
        encode_state(&self.doc)
    }

    pub fn apply(&mut self, batch: Batch) -> Result<EditorUpdate, EditorError> {
        self.apply_committing(batch, |_, _, _| Ok(()))
    }

    pub(crate) fn apply_committing<E: From<EditorError>>(
        &mut self,
        batch: Batch,
        commit: impl FnOnce(&Snapshot, &Doc, &EditorUpdate) -> Result<(), E>,
    ) -> Result<EditorUpdate, E> {
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
        let history = if !batch.external && !patches.is_empty() {
            Some(HistoryEntry {
                order: [
                    insertion_order(&self.snapshot, &patches),
                    insertion_order(&staged, &reverse),
                ],
                forward: patches.clone(),
                reverse,
            })
        } else {
            None
        };
        let update = self.publish(staged, doc, patches, commit)?;
        if let Some(history) = history {
            self.undo.push(history);
            self.redo.clear();
        }
        Ok(update)
    }

    pub fn history(&mut self, revision: u64, redo: bool) -> Result<EditorUpdate, EditorError> {
        self.history_committing(revision, redo, |_, _, _| Ok(()))
    }

    pub(crate) fn history_committing<E: From<EditorError>>(
        &mut self,
        revision: u64,
        redo: bool,
        commit: impl FnOnce(&Snapshot, &Doc, &EditorUpdate) -> Result<(), E>,
    ) -> Result<EditorUpdate, E> {
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
                _ => return Err(EditorError::HistoryConflict.into()),
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
                        let index = entry.order[usize::from(redo)]
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
        let update = self.publish(staged, doc, patches, commit)?;
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
        Ok((self.snapshot.clone(), load_doc(&self.crdt_state())?))
    }

    fn publish<E: From<EditorError>>(
        &mut self,
        mut staged: Snapshot,
        doc: Doc,
        patches: Vec<Patch>,
        commit: impl FnOnce(&Snapshot, &Doc, &EditorUpdate) -> Result<(), E>,
    ) -> Result<EditorUpdate, E> {
        let base_revision = self.snapshot.revision;
        if patches.is_empty() {
            return Ok(self.unchanged());
        }
        staged.revision = base_revision
            .checked_add(1)
            .ok_or(EditorError::InvalidOperation)?;
        write_layout(&doc, &staged)?;
        let update = EditorUpdate {
            base_revision,
            revision: staged.revision,
            patches,
        };
        commit(&staged, &doc, &update)?;
        self.snapshot = staged;
        self.doc = doc;
        Ok(update)
    }
}

pub(crate) fn encode_state(doc: &Doc) -> Vec<u8> {
    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

fn insertion_order(snapshot: &Snapshot, opposite: &[Patch]) -> Vec<String> {
    if opposite
        .iter()
        .any(|patch| matches!(patch, Patch::Delete { .. }))
    {
        snapshot
            .blocks
            .iter()
            .map(|block| block.id.clone())
            .collect()
    } else {
        Vec::new()
    }
}

fn load_doc(state: &[u8]) -> Result<Doc, EditorError> {
    let doc = text_doc();
    let update = Update::decode_v1(state).map_err(|_| EditorError::InvalidDocument)?;
    doc.transact_mut()
        .apply_update(update)
        .map_err(|_| EditorError::InvalidDocument)?;
    Ok(doc)
}

fn write_layout(doc: &Doc, snapshot: &Snapshot) -> Result<(), EditorError> {
    let layout = serde_json::to_string(snapshot).map_err(|_| EditorError::InvalidDocument)?;
    let map = doc.get_or_insert_map("document");
    map.insert(&mut doc.transact_mut(), "layout", layout);
    Ok(())
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
            BlockKind::Paragraph { content } | BlockKind::Styled { content, .. } => {
                content.is_valid()
            }
            BlockKind::Checklist { content, depth, .. } => *depth <= 6 && content.is_valid(),
            BlockKind::Table { rows } => rows.first().is_some_and(|first| {
                !first.is_empty()
                    && rows
                        .iter()
                        .all(|row| row.len() == first.len() && row.iter().all(RichText::is_valid))
            }),
            BlockKind::Image { attachment_id } => !attachment_id.is_empty(),
            BlockKind::NoteLink { note_id, .. } => !note_id.is_empty(),
        }
    }

    fn write_text(&self, doc: &Doc) {
        match &self.kind {
            BlockKind::Paragraph { content }
            | BlockKind::Checklist { content, .. }
            | BlockKind::Styled { content, .. } => {
                content.write(doc, &text_key(&self.id, None));
            }
            BlockKind::Image { .. } | BlockKind::NoteLink { .. } => (),
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
            (
                BlockKind::Paragraph { content }
                | BlockKind::Checklist { content, .. }
                | BlockKind::Styled { content, .. },
                None,
            ) => Ok(content),
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
            for style in [
                Style::Bold,
                Style::Italic,
                Style::Underline,
                Style::Strikethrough,
            ] {
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
        | Operation::SplitParagraph { id, .. }
        | Operation::MergePrevious { id }
        | Operation::Indent { id, .. }
        | Operation::Toggle { id }
        | Operation::Format { id, .. }
        | Operation::ParagraphStyle { id, .. }
        | Operation::InsertAt { id, .. }
        | Operation::RemoveEmbed { id }
        | Operation::TableInsertRow { id, .. }
        | Operation::TableRemoveRow { id, .. }
        | Operation::TableInsertColumn { id, .. }
        | Operation::TableRemoveColumn { id, .. }
        | Operation::TableMoveRow { id, .. }
        | Operation::TableMoveColumn { id, .. } => id,
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
        Operation::SplitParagraph { offset, new_id, .. } => {
            if new_id.is_empty() || blocks.iter().any(|block| block.id == new_id) {
                return Err(EditorError::InvalidOperation);
            }
            let source = &mut blocks[index];
            let style = match source.kind {
                BlockKind::Styled {
                    style:
                        style @ (ParagraphStyle::Bullet
                        | ParagraphStyle::Dashed
                        | ParagraphStyle::Numbered
                        | ParagraphStyle::Monospaced
                        | ParagraphStyle::Quote),
                    ..
                } => Some(style),
                _ => None,
            };
            let tail = split_content(source.content_mut(None)?, offset, offset)?;
            source.write_text(doc);
            let next = Block {
                id: new_id,
                kind: match style {
                    Some(style) => BlockKind::Styled {
                        content: tail,
                        style,
                    },
                    None => BlockKind::Paragraph { content: tail },
                },
            };
            next.write_text(doc);
            blocks.insert(index + 1, next);
        }
        Operation::MergePrevious { .. } => {
            let previous = index.checked_sub(1).ok_or(EditorError::InvalidTarget)?;
            let (earlier, later) = blocks.split_at_mut(index);
            let prefix = earlier[previous].content_mut(None)?;
            let suffix = later[0].content_mut(None)?;
            let offset = byte_to_utf16(&prefix.text, prefix.text.len())?;
            prefix.text.push_str(&suffix.text);
            prefix.marks.extend(suffix.marks.iter().map(|mark| Mark {
                start: mark.start + offset,
                end: mark.end + offset,
                style: mark.style,
            }));
            earlier[previous].write_text(doc);
            blocks.remove(index);
        }
        Operation::Toggle { .. } => {
            let BlockKind::Checklist { checked, .. } = &mut blocks[index].kind else {
                return Err(EditorError::InvalidOperation);
            };
            *checked = !*checked;
        }
        Operation::Format {
            id,
            cell,
            start,
            end,
            style,
        } => {
            let content = blocks[index].content_mut(cell)?;
            if start >= end {
                return Err(EditorError::InvalidRange);
            }
            utf16_to_byte(&content.text, start)?;
            utf16_to_byte(&content.text, end)?;
            let enabled = !content
                .marks
                .iter()
                .any(|mark| mark.style == style && mark.start <= start && mark.end >= end);
            let key = text_key(&id, cell);
            let target = doc.get_or_insert_text(key.as_str());
            target.format(
                &mut doc.transact_mut(),
                start,
                end - start,
                [(
                    style.key().into(),
                    if enabled { Any::Bool(true) } else { Any::Null },
                )]
                .into(),
            );
            *content = project(doc, &key);
        }
        Operation::ParagraphStyle {
            style, checklist, ..
        } => {
            let item = &mut blocks[index];
            if checklist && matches!(item.kind, BlockKind::Checklist { .. }) {
                return Ok(());
            }
            let content = item.content_mut(None)?.clone();
            item.kind = match (style, checklist) {
                (_, true) => BlockKind::Checklist {
                    content,
                    depth: 0,
                    checked: false,
                },
                (Some(style), false) => BlockKind::Styled { content, style },
                (None, false) => BlockKind::Paragraph { content },
            };
        }
        Operation::InsertAt {
            start,
            end,
            block,
            tail_id,
            ..
        } => {
            if block.id.is_empty()
                || tail_id.is_empty()
                || block.id == tail_id
                || blocks
                    .iter()
                    .any(|item| item.id == block.id || item.id == tail_id)
                || !block.is_valid()
                || !matches!(
                    block.kind,
                    BlockKind::Image { .. } | BlockKind::NoteLink { .. } | BlockKind::Table { .. }
                )
            {
                return Err(EditorError::InvalidOperation);
            }
            let source = &mut blocks[index];
            let tail = split_content(source.content_mut(None)?, start, end)?;
            source.write_text(doc);
            let tail = Block {
                id: tail_id,
                kind: BlockKind::Paragraph { content: tail },
            };
            tail.write_text(doc);
            // Bảng mới phải ghi nội dung các ô vào khóa vị trí trong CRDT.
            block.write_text(doc);
            blocks.splice(index + 1..index + 1, [block, tail]);
        }
        Operation::RemoveEmbed { .. } => {
            if !matches!(
                blocks[index].kind,
                BlockKind::Image { .. } | BlockKind::NoteLink { .. }
            ) {
                return Err(EditorError::InvalidOperation);
            }
            blocks.remove(index);
        }
        Operation::TableInsertRow { after_row, .. } => {
            let BlockKind::Table { rows } = &mut blocks[index].kind else {
                return Err(EditorError::InvalidOperation);
            };
            let columns = rows.first().map(Vec::len).ok_or(EditorError::InvalidTarget)?;
            let position = match after_row {
                Some(row) if row < rows.len() => row + 1,
                None => rows.len(),
                _ => return Err(EditorError::InvalidTarget),
            };
            rows.insert(
                position,
                (0..columns).map(|_| RichText::default()).collect(),
            );
            // Khóa ô trong CRDT mang số vị trí (row:column) nên sau khi đổi
            // cấu trúc phải ghi lại toàn bộ các ô để khớp khóa mới.
            blocks[index].write_text(doc);
        }
        Operation::TableRemoveRow { row, .. } => {
            let BlockKind::Table { rows } = &mut blocks[index].kind else {
                return Err(EditorError::InvalidOperation);
            };
            if rows.len() <= 1 || row >= rows.len() {
                return Err(EditorError::InvalidTarget);
            }
            rows.remove(row);
            blocks[index].write_text(doc);
        }
        Operation::TableInsertColumn { after_column, .. } => {
            let BlockKind::Table { rows } = &mut blocks[index].kind else {
                return Err(EditorError::InvalidOperation);
            };
            let columns = rows.first().map(Vec::len).ok_or(EditorError::InvalidTarget)?;
            let position = match after_column {
                Some(column) if column < columns => column + 1,
                None => columns,
                _ => return Err(EditorError::InvalidTarget),
            };
            for row in rows.iter_mut() {
                row.insert(position, RichText::default());
            }
            blocks[index].write_text(doc);
        }
        Operation::TableRemoveColumn { column, .. } => {
            let BlockKind::Table { rows } = &mut blocks[index].kind else {
                return Err(EditorError::InvalidOperation);
            };
            let columns = rows.first().map(Vec::len).ok_or(EditorError::InvalidTarget)?;
            if columns <= 1 || column >= columns {
                return Err(EditorError::InvalidTarget);
            }
            for row in rows.iter_mut() {
                row.remove(column);
            }
            blocks[index].write_text(doc);
        }
        Operation::TableMoveRow { from, to, .. } => {
            let BlockKind::Table { rows } = &mut blocks[index].kind else {
                return Err(EditorError::InvalidOperation);
            };
            if from >= rows.len() || to >= rows.len() || from == to {
                return Err(EditorError::InvalidTarget);
            }
            let row = rows.remove(from);
            rows.insert(to, row);
            blocks[index].write_text(doc);
        }
        Operation::TableMoveColumn { from, to, .. } => {
            let BlockKind::Table { rows } = &mut blocks[index].kind else {
                return Err(EditorError::InvalidOperation);
            };
            let columns = rows.first().map(Vec::len).ok_or(EditorError::InvalidTarget)?;
            if from >= columns || to >= columns || from == to {
                return Err(EditorError::InvalidTarget);
            }
            for row in rows.iter_mut() {
                let cell = row.remove(from);
                row.insert(to, cell);
            }
            blocks[index].write_text(doc);
        }
    }
    Ok(())
}

fn split_content(content: &mut RichText, start: u32, end: u32) -> Result<RichText, EditorError> {
    let start_byte = utf16_to_byte(&content.text, start)?;
    let end_byte = utf16_to_byte(&content.text, end)?;
    if start_byte > end_byte {
        return Err(EditorError::InvalidRange);
    }
    let tail = RichText {
        text: content.text[end_byte..].to_owned(),
        marks: content
            .marks
            .iter()
            .filter(|mark| mark.end > end)
            .map(|mark| Mark {
                start: mark.start.saturating_sub(end),
                end: mark.end - end,
                style: mark.style,
            })
            .collect(),
    };
    content.text.truncate(start_byte);
    content.marks.retain_mut(|mark| {
        mark.end = mark.end.min(start);
        mark.start < mark.end
    });
    Ok(tail)
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
