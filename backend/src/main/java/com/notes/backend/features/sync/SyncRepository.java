package com.notes.backend.features.sync;

import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.jdbc.core.simple.JdbcClient;
import org.springframework.stereotype.Repository;

@Repository
public class SyncRepository {
    private final JdbcClient jdbc;

    public SyncRepository(JdbcClient jdbc) {
        this.jdbc = jdbc;
    }

    public long lockUserLastSyncSeq(UUID userId) {
        return jdbc.sql("SELECT last_sync_seq FROM users WHERE id = :userId FOR UPDATE")
            .param("userId", userId)
            .query(Long.class)
            .single();
    }

    public Optional<DeltaRow> findByClientOpId(UUID userId, UUID clientOpId) {
        return jdbc.sql("""
            SELECT server_seq, device_id, entity_type, entity_id, client_op_id,
                   crypto_version, key_version, encrypted_payload, created_at
            FROM sync_deltas
            WHERE user_id = :userId AND client_op_id = :clientOpId
            """)
            .param("userId", userId)
            .param("clientOpId", clientOpId)
            .query((rs, rowNum) -> new DeltaRow(
                rs.getLong("server_seq"),
                rs.getObject("device_id", UUID.class),
                rs.getString("entity_type"),
                rs.getObject("entity_id", UUID.class),
                rs.getObject("client_op_id", UUID.class),
                rs.getShort("crypto_version"),
                rs.getInt("key_version"),
                rs.getBytes("encrypted_payload"),
                rs.getLong("created_at")
            ))
            .optional();
    }

    public void insertDelta(
        long serverSeq,
        UUID userId,
        UUID deviceId,
        String entityType,
        UUID entityId,
        UUID clientOpId,
        short cryptoVersion,
        int keyVersion,
        byte[] encryptedPayload,
        long createdAt
    ) {
        jdbc.sql("""
            INSERT INTO sync_deltas (
                server_seq, user_id, device_id, entity_type, entity_id,
                client_op_id, crypto_version, key_version, encrypted_payload, created_at
            ) VALUES (
                :serverSeq, :userId, :deviceId, :entityType, :entityId,
                :clientOpId, :cryptoVersion, :keyVersion, :encryptedPayload, :createdAt
            )
            """)
            .param("serverSeq", serverSeq)
            .param("userId", userId)
            .param("deviceId", deviceId)
            .param("entityType", entityType)
            .param("entityId", entityId)
            .param("clientOpId", clientOpId)
            .param("cryptoVersion", cryptoVersion)
            .param("keyVersion", keyVersion)
            .param("encryptedPayload", encryptedPayload)
            .param("createdAt", createdAt)
            .update();
    }

    public void updateLastSyncSeq(UUID userId, long lastSyncSeq) {
        jdbc.sql("UPDATE users SET last_sync_seq = :seq WHERE id = :userId")
            .param("seq", lastSyncSeq)
            .param("userId", userId)
            .update();
    }

    public List<DeltaRow> pullSince(UUID userId, long sinceSeq, int limitPlusOne) {
        return jdbc.sql("""
            SELECT server_seq, device_id, entity_type, entity_id, client_op_id,
                   crypto_version, key_version, encrypted_payload, created_at
            FROM sync_deltas
            WHERE user_id = :userId AND server_seq > :sinceSeq
            ORDER BY server_seq ASC
            LIMIT :limitPlusOne
            """)
            .param("userId", userId)
            .param("sinceSeq", sinceSeq)
            .param("limitPlusOne", limitPlusOne)
            .query((rs, rowNum) -> new DeltaRow(
                rs.getLong("server_seq"),
                rs.getObject("device_id", UUID.class),
                rs.getString("entity_type"),
                rs.getObject("entity_id", UUID.class),
                rs.getObject("client_op_id", UUID.class),
                rs.getShort("crypto_version"),
                rs.getInt("key_version"),
                rs.getBytes("encrypted_payload"),
                rs.getLong("created_at")
            ))
            .list();
    }

    public record DeltaRow(
        long serverSeq,
        UUID deviceId,
        String entityType,
        UUID entityId,
        UUID clientOpId,
        short cryptoVersion,
        int keyVersion,
        byte[] encryptedPayload,
        long createdAt
    ) {}
}
