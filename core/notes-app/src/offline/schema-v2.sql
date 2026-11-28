CREATE VIRTUAL TABLE notes_fts USING fts5(
    note_id UNINDEXED,
    title,
    body_folded,
    tokenize = 'unicode61 remove_diacritics 2'
);
