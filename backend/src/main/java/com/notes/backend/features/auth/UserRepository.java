package com.notes.backend.features.auth;

import com.notes.backend.features.auth.model.UserRecord;
import java.util.Optional;
import java.util.UUID;
import org.springframework.jdbc.core.simple.JdbcClient;
import org.springframework.stereotype.Repository;

@Repository
public class UserRepository {
    private final JdbcClient jdbc;

    public UserRepository(JdbcClient jdbc) {
        this.jdbc = jdbc;
    }

    public Optional<UserRecord> findByEmailHash(String emailHash) {
        return jdbc.sql("""
            SELECT id, email_hash, auth_verifier, kdf_salt, kdf_params_json::text AS kdf_params_json,
                   wrapped_amk, recovery_wrapped_amk, recovery_auth_verifier, key_ring_encrypted,
                   key_version, last_sync_seq, created_at
            FROM users WHERE email_hash = :emailHash
            """)
            .param("emailHash", emailHash)
            .query((rs, rowNum) -> new UserRecord(
                rs.getObject("id", UUID.class),
                rs.getString("email_hash"),
                rs.getString("auth_verifier"),
                rs.getBytes("kdf_salt"),
                rs.getString("kdf_params_json"),
                rs.getBytes("wrapped_amk"),
                rs.getBytes("recovery_wrapped_amk"),
                rs.getString("recovery_auth_verifier"),
                rs.getBytes("key_ring_encrypted"),
                rs.getInt("key_version"),
                rs.getLong("last_sync_seq"),
                rs.getLong("created_at")
            ))
            .optional();
    }

    public Optional<UserRecord> findById(UUID id) {
        return jdbc.sql("""
            SELECT id, email_hash, auth_verifier, kdf_salt, kdf_params_json::text AS kdf_params_json,
                   wrapped_amk, recovery_wrapped_amk, recovery_auth_verifier, key_ring_encrypted,
                   key_version, last_sync_seq, created_at
            FROM users WHERE id = :id
            """)
            .param("id", id)
            .query((rs, rowNum) -> new UserRecord(
                rs.getObject("id", UUID.class),
                rs.getString("email_hash"),
                rs.getString("auth_verifier"),
                rs.getBytes("kdf_salt"),
                rs.getString("kdf_params_json"),
                rs.getBytes("wrapped_amk"),
                rs.getBytes("recovery_wrapped_amk"),
                rs.getString("recovery_auth_verifier"),
                rs.getBytes("key_ring_encrypted"),
                rs.getInt("key_version"),
                rs.getLong("last_sync_seq"),
                rs.getLong("created_at")
            ))
            .optional();
    }

    public void insert(UserRecord user) {
        jdbc.sql("""
            INSERT INTO users (
                id, email_hash, auth_verifier, kdf_salt, kdf_params_json,
                wrapped_amk, recovery_wrapped_amk, recovery_auth_verifier,
                key_ring_encrypted, key_version, last_sync_seq, created_at
            ) VALUES (
                :id, :emailHash, :authVerifier, :kdfSalt, CAST(:kdfParamsJson AS jsonb),
                :wrappedAmk, :recoveryWrappedAmk, :recoveryAuthVerifier,
                :keyRingEncrypted, :keyVersion, :lastSyncSeq, :createdAt
            )
            """)
            .param("id", user.id())
            .param("emailHash", user.emailHash())
            .param("authVerifier", user.authVerifier())
            .param("kdfSalt", user.kdfSalt())
            .param("kdfParamsJson", user.kdfParamsJson())
            .param("wrappedAmk", user.wrappedAmk())
            .param("recoveryWrappedAmk", user.recoveryWrappedAmk())
            .param("recoveryAuthVerifier", user.recoveryAuthVerifier())
            .param("keyRingEncrypted", user.keyRingEncrypted())
            .param("keyVersion", user.keyVersion())
            .param("lastSyncSeq", user.lastSyncSeq())
            .param("createdAt", user.createdAt())
            .update();
    }

    public void updateCredentials(
        UUID userId,
        String authVerifier,
        byte[] kdfSalt,
        String kdfParamsJson,
        byte[] wrappedAmk
    ) {
        jdbc.sql("""
            UPDATE users SET
                auth_verifier = :authVerifier,
                kdf_salt = :kdfSalt,
                kdf_params_json = CAST(:kdfParamsJson AS jsonb),
                wrapped_amk = :wrappedAmk
            WHERE id = :id
            """)
            .param("id", userId)
            .param("authVerifier", authVerifier)
            .param("kdfSalt", kdfSalt)
            .param("kdfParamsJson", kdfParamsJson)
            .param("wrappedAmk", wrappedAmk)
            .update();
    }

    public int rotateKey(
        UUID userId,
        int expectedKeyVersion,
        byte[] newWrappedAmk,
        byte[] recoveryWrappedAmk,
        byte[] keyRingEncrypted
    ) {
        return jdbc.sql("""
            UPDATE users SET
                wrapped_amk = :wrappedAmk,
                recovery_wrapped_amk = COALESCE(:recoveryWrappedAmk, recovery_wrapped_amk),
                key_ring_encrypted = :keyRingEncrypted,
                key_version = key_version + 1
            WHERE id = :id AND key_version = :expectedKeyVersion
            """)
            .param("id", userId)
            .param("expectedKeyVersion", expectedKeyVersion)
            .param("wrappedAmk", newWrappedAmk)
            .param("recoveryWrappedAmk", recoveryWrappedAmk)
            .param("keyRingEncrypted", keyRingEncrypted)
            .update();
    }
}
