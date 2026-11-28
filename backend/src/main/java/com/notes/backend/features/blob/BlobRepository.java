package com.notes.backend.features.blob;

import java.util.Optional;
import java.util.UUID;
import org.springframework.jdbc.core.simple.JdbcClient;
import org.springframework.stereotype.Repository;

@Repository
public class BlobRepository {
    private final JdbcClient jdbc;

    public BlobRepository(JdbcClient jdbc) {
        this.jdbc = jdbc;
    }

    public Optional<UserBlobRow> findUserBlob(UUID userId, String contentHash) {
        return jdbc.sql("""
            SELECT byte_size, storage_key, ciphertext_sha256
            FROM user_blobs
            WHERE user_id = :userId AND content_hash = :contentHash
            """)
            .param("userId", userId)
            .param("contentHash", contentHash)
            .query((rs, rowNum) -> new UserBlobRow(
                rs.getLong("byte_size"),
                rs.getString("storage_key"),
                rs.getString("ciphertext_sha256")
            ))
            .optional();
    }

    public void insertUserBlob(
        UUID userId,
        String contentHash,
        long byteSize,
        String storageKey,
        String ciphertextSha256,
        long now
    ) {
        jdbc.sql("""
            INSERT INTO user_blobs (user_id, content_hash, byte_size, storage_key, ciphertext_sha256, ref_count, created_at)
            VALUES (:userId, :contentHash, :byteSize, :storageKey, :ciphertextSha256, 1, :now)
            ON CONFLICT (user_id, content_hash) DO NOTHING
            """)
            .param("userId", userId)
            .param("contentHash", contentHash)
            .param("byteSize", byteSize)
            .param("storageKey", storageKey)
            .param("ciphertextSha256", ciphertextSha256)
            .param("now", now)
            .update();
    }

    public void insertUploadToken(
        String token,
        UUID userId,
        String contentHash,
        long byteSize,
        String ciphertextSha256,
        String stagingKey,
        String finalKey,
        long expiresAt
    ) {
        jdbc.sql("""
            INSERT INTO blob_upload_tokens (
                token, user_id, content_hash, byte_size, ciphertext_sha256,
                staging_key, final_key, expires_at, completed_at
            ) VALUES (
                :token, :userId, :contentHash, :byteSize, :ciphertextSha256,
                :stagingKey, :finalKey, :expiresAt, NULL
            )
            """)
            .param("token", token)
            .param("userId", userId)
            .param("contentHash", contentHash)
            .param("byteSize", byteSize)
            .param("ciphertextSha256", ciphertextSha256)
            .param("stagingKey", stagingKey)
            .param("finalKey", finalKey)
            .param("expiresAt", expiresAt)
            .update();
    }

    public Optional<UploadTokenRow> findUploadToken(String token, UUID userId) {
        return jdbc.sql("""
            SELECT content_hash, byte_size, ciphertext_sha256, staging_key, final_key, expires_at, completed_at
            FROM blob_upload_tokens
            WHERE token = :token AND user_id = :userId
            """)
            .param("token", token)
            .param("userId", userId)
            .query((rs, rowNum) -> new UploadTokenRow(
                rs.getString("content_hash"),
                rs.getLong("byte_size"),
                rs.getString("ciphertext_sha256"),
                rs.getString("staging_key"),
                rs.getString("final_key"),
                rs.getLong("expires_at"),
                rs.getObject("completed_at", Long.class)
            ))
            .optional();
    }

    public void markTokenCompleted(String token, long now) {
        jdbc.sql("UPDATE blob_upload_tokens SET completed_at = :now WHERE token = :token AND completed_at IS NULL")
            .param("now", now)
            .param("token", token)
            .update();
    }

    public record UserBlobRow(long byteSize, String storageKey, String ciphertextSha256) {}

    public record UploadTokenRow(
        String contentHash,
        long byteSize,
        String ciphertextSha256,
        String stagingKey,
        String finalKey,
        long expiresAt,
        Long completedAt
    ) {}
}
