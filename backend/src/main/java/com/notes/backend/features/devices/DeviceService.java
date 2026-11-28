package com.notes.backend.features.devices;

import com.notes.backend.common.config.NotesProperties;
import com.notes.backend.common.exception.DomainException;
import com.notes.backend.common.security.DeviceStatusChecker;
import com.notes.backend.features.devices.dto.DeviceDto;
import com.notes.backend.features.devices.dto.DeviceListResponse;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.security.SecureRandom;
import java.time.Instant;
import java.util.Base64;
import java.util.HexFormat;
import java.util.UUID;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

@Service
public class DeviceService implements DeviceStatusChecker {
    private static final SecureRandom RNG = new SecureRandom();
    private final DeviceRepository deviceRepository;
    private final long refreshTokenTtlMillis;

    public DeviceService(DeviceRepository deviceRepository, NotesProperties properties) {
        this.deviceRepository = deviceRepository;
        this.refreshTokenTtlMillis = properties.security().refreshTokenTtlSeconds() * 1000L;
    }

    @Override
    public boolean isActiveDevice(UUID userId, UUID deviceId) {
        return deviceRepository.isActiveDevice(userId, deviceId);
    }

    @Transactional
    public String registerDeviceAndIssueRefreshToken(
        UUID userId,
        UUID deviceId,
        String deviceName,
        String platform
    ) {
        long now = Instant.now().toEpochMilli();
        deviceRepository.upsertDevice(deviceId, userId, deviceName, platform, now);
        return createAndStoreRefreshToken(userId, deviceId, now);
    }

    @Transactional
    public RotatedSession rotateRefreshToken(String rawRefreshToken, UUID expectedDeviceId) {
        long now = Instant.now().toEpochMilli();
        String hash = sha256Hex(rawRefreshToken);
        var row = deviceRepository.findValidRefreshToken(hash, now)
            .filter(r -> r.deviceId().equals(expectedDeviceId))
            .orElseThrow(() -> new DomainException(
                HttpStatus.UNAUTHORIZED,
                "invalid_refresh_token",
                "Phiên làm việc đã hết hạn hoặc bị thu hồi"
            ));
        deviceRepository.revokeRefreshToken(hash, now);
        deviceRepository.touchDevice(row.userId(), row.deviceId(), now);
        String nextToken = createAndStoreRefreshToken(row.userId(), row.deviceId(), now);
        return new RotatedSession(row.userId(), row.deviceId(), nextToken);
    }

    @Transactional
    public void revokeOtherDevicesRefreshTokens(UUID userId, UUID currentDeviceId) {
        deviceRepository.revokeOtherRefreshTokens(userId, currentDeviceId, Instant.now().toEpochMilli());
    }

    public DeviceListResponse listDevices(UUID userId, UUID currentDeviceId) {
        var list = deviceRepository.findByUserId(userId).stream()
            .map(d -> new DeviceDto(
                d.id(),
                d.deviceName(),
                d.platform(),
                d.lastSeenAt(),
                d.revokedAt(),
                d.createdAt(),
                d.id().equals(currentDeviceId)
            ))
            .toList();
        return new DeviceListResponse(list);
    }

    @Transactional
    public void revokeDevice(UUID userId, UUID targetDeviceId) {
        long now = Instant.now().toEpochMilli();
        int updated = deviceRepository.revokeDevice(userId, targetDeviceId, now);
        if (updated == 0) {
            throw new DomainException(
                HttpStatus.NOT_FOUND,
                "device_not_found",
                "Không tìm thấy thiết bị đang hoạt động"
            );
        }
    }

    private String createAndStoreRefreshToken(UUID userId, UUID deviceId, long now) {
        byte[] bytes = new byte[32];
        RNG.nextBytes(bytes);
        String raw = Base64.getUrlEncoder().withoutPadding().encodeToString(bytes);
        deviceRepository.insertRefreshToken(
            sha256Hex(raw),
            userId,
            deviceId,
            now + refreshTokenTtlMillis,
            now
        );
        return raw;
    }

    private static String sha256Hex(String raw) {
        try {
            byte[] digest = MessageDigest.getInstance("SHA-256")
                .digest(raw.getBytes(StandardCharsets.UTF_8));
            return HexFormat.of().formatHex(digest);
        } catch (NoSuchAlgorithmException ex) {
            throw new IllegalStateException(ex);
        }
    }

    public record RotatedSession(UUID userId, UUID deviceId, String refreshToken) {}
}
