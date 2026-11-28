-- v6: Tổ chức nâng cao — tag inline, Smart Folders và FTS có cột tags.
CREATE TABLE note_tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY (note_id, tag)
);
CREATE INDEX idx_note_tags_tag ON note_tags(tag);
ALTER TABLE folders ADD COLUMN smart_query_json TEXT;
-- FTS5 không ALTER được: dựng lại với cột tags và nạp lại ở Rust sau migration.
DROP TABLE IF EXISTS notes_fts;
CREATE VIRTUAL TABLE notes_fts USING fts5(
    note_id UNINDEXED,
    title,
    body_folded,
    tags,
    tokenize = 'unicode61 remove_diacritics 2'
);
