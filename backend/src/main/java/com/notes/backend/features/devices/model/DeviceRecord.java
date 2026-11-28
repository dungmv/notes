package com.notes.backend.features.devices.model;

import java.util.UUID;

public record DeviceRecord(
    UUID id,
    UUID userId,
    String deviceName,
    String platform,
    long lastSeenAt,
    Long revokedAt,
    long createdAt
) {}
