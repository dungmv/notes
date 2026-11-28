CREATE TABLE preferences (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    auto_sort_checklist INTEGER NOT NULL CHECK(auto_sort_checklist IN (0, 1))
);
INSERT INTO preferences VALUES(1, 0);
