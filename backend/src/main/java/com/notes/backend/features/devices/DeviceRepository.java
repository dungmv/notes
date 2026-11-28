package com.notes.backend.features.devices;

import com.notes.backend.features.devices.model.DeviceRecord;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.jdbc.core.simple.JdbcClient;
import org.springframework.stereotype.Repository;

@Repository
public class DeviceRepository {
    private final JdbcClient jdbc;

    public DeviceRepository(JdbcClient jdbc) {
        this.jdbc = jdbc;
    }

    public boolean isActiveDevice(UUID userId, UUID deviceId) {
        return jdbc.sql("""
            SELECT COUNT(*) FROM devices
            WHERE id = :id AND user_id = :userId AND revoked_at IS NULL
            """)
            .param("id", deviceId)
            .param("userId", userId)
            .query(Long.class)
            .single() > 0;
    }

    public void upsertDevice(
        UUID deviceId,
        UUID userId,
        String deviceName,
        String platform,
        long now
    ) {
        jdbc.sql("""
            INSERT INTO devices (id, user_id, device_name, platform, last_seen_at, revoked_at, created_at)
            VALUES (:id, :userId, :deviceName, :platform, :now, NULL, :now)
            ON CONFLICT (id) DO UPDATE SET
                device_name = EXCLUDED.device_name,
                platform = EXCLUDED.platform,
                last_seen_at = EXCLUDED.last_seen_at,
                revoked_at = NULL
            WHERE devices.user_id = EXCLUDED.user_id
            """)
            .param("id", deviceId)
            .param("userId", userId)
            .param("deviceName", deviceName)
            .param("platform", platform)
            .param("now", now)
            .update();
    }

    public void touchDevice(UUID userId, UUID deviceId, long now) {
        jdbc.sql("UPDATE devices SET last_seen_at = :now WHERE id = :id AND user_id = :userId")
            .param("now", now)
            .param("id", deviceId)
            .param("userId", userId)
            .update();
    }

    public List<DeviceRecord> findByUserId(UUID userId) {
        return jdbc.sql("""
            SELECT id, user_id, device_name, platform, last_seen_at, revoked_at, created_at
            FROM devices WHERE user_id = :userId
            ORDER BY created_at ASC
            """)
            .param("userId", userId)
            .query((rs, rowNum) -> new DeviceRecord(
                rs.getObject("id", UUID.class),
                rs.getObject("user_id", UUID.class),
                rs.getString("device_name"),
                rs.getString("platform"),
                rs.getLong("last_seen_at"),
                rs.getObject("revoked_at", Long.class),
                rs.getLong("created_at")
            ))
            .list();
    }

    public int revokeDevice(UUID userId, UUID deviceId, long now) {
        int updated = jdbc.sql("""
            UPDATE devices SET revoked_at = :now
            WHERE id = :id AND user_id = :userId AND revoked_at IS NULL
            """)
            .param("now", now)
            .param("id", deviceId)
            .param("userId", userId)
            .update();
        jdbc.sql("""
            UPDATE refresh_tokens SET revoked_at = :now
            WHERE device_id = :deviceId AND user_id = :userId AND revoked_at IS NULL
            """)
            .param("now", now)
            .param("deviceId", deviceId)
            .param("userId", userId)
            .update();
        return updated;
    }

    public void insertRefreshToken(
        String tokenHash,
        UUID userId,
        UUID deviceId,
        long expiresAt,
        long now
    ) {
        jdbc.sql("""
            INSERT INTO refresh_tokens (token_hash, user_id, device_id, expires_at, revoked_at, created_at)
            VALUES (:hash, :userId, :deviceId, :expiresAt, NULL, :now)
            """)
            .param("hash", tokenHash)
            .param("userId", userId)
            .param("deviceId", deviceId)
            .param("expiresAt", expiresAt)
            .param("now", now)
            .update();
    }

    public Optional<RefreshTokenRow> findValidRefreshToken(String tokenHash, long now) {
        return jdbc.sql("""
            SELECT rt.user_id, rt.device_id
            FROM refresh_tokens rt
            JOIN devices d ON d.id = rt.device_id AND d.user_id = rt.user_id
            WHERE rt.token_hash = :hash
              AND rt.revoked_at IS NULL
              AND rt.expires_at > :now
              AND d.revoked_at IS NULL
            """)
            .param("hash", tokenHash)
            .param("now", now)
            .query((rs, rowNum) -> new RefreshTokenRow(
                rs.getObject("user_id", UUID.class),
                rs.getObject("device_id", UUID.class)
            ))
            .optional();
    }

    public void revokeRefreshToken(String tokenHash, long now) {
        jdbc.sql("""
            UPDATE refresh_tokens SET revoked_at = :now
            WHERE token_hash = :hash AND revoked_at IS NULL
            """)
            .param("now", now)
            .param("hash", tokenHash)
            .update();
    }

    public void revokeOtherRefreshTokens(UUID userId, UUID keepDeviceId, long now) {
        jdbc.sql("""
            UPDATE refresh_tokens SET revoked_at = :now
            WHERE user_id = :userId AND device_id <> :keepDeviceId AND revoked_at IS NULL
            """)
            .param("now", now)
            .param("userId", userId)
            .param("keepDeviceId", keepDeviceId)
            .update();
    }

    public record RefreshTokenRow(UUID userId, UUID deviceId) {}
}
