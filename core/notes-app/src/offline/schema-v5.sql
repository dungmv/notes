-- v5: Locked Notes cục bộ. Metadata khóa và manifest attachment đã mã hóa
-- nằm cùng dòng ghi chú; crdt_state giữ nguyên cột nhưng chứa ciphertext.
ALTER TABLE notes ADD COLUMN lock_metadata BLOB;
ALTER TABLE notes ADD COLUMN lock_manifest BLOB;
