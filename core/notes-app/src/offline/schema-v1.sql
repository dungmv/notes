CREATE TABLE folders (
    id TEXT PRIMARY KEY NOT NULL,
    parent_id TEXT REFERENCES folders(id),
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('all','inbox','trash','normal')),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);

INSERT INTO folders(id,name,kind,sort_order,created_at,updated_at) VALUES
    ('all','Tất cả ghi chú','all',0,0,0),
    ('inbox','Ghi chú','inbox',1,0,0),
    ('trash','Đã xóa gần đây','trash',3,0,0);

CREATE TABLE notes (
    id TEXT PRIMARY KEY NOT NULL,
    folder_id TEXT NOT NULL REFERENCES folders(id),
    title TEXT NOT NULL DEFAULT '',
    snippet TEXT NOT NULL DEFAULT '',
    is_pinned INTEGER NOT NULL DEFAULT 0,
    is_locked INTEGER NOT NULL DEFAULT 0,
    crdt_state BLOB NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0,
    plain_text_cache TEXT NOT NULL DEFAULT '',
    has_checklist INTEGER NOT NULL DEFAULT 0,
    has_attachments INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);

CREATE INDEX idx_notes_folder_updated ON notes(folder_id,is_pinned DESC,updated_at DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX idx_notes_deleted ON notes(deleted_at) WHERE deleted_at IS NOT NULL;

CREATE TABLE attachments (
    id TEXT PRIMARY KEY NOT NULL,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    file_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_attachments_note ON attachments(note_id);
