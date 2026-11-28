package com.notes.backend.features.devices.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.UUID;

public record DeviceDto(
    UUID id,
    @JsonProperty("device_name") String deviceName,
    String platform,
    @JsonProperty("last_seen_at") long lastSeenAt,
    @JsonProperty("revoked_at") Long revokedAt,
    @JsonProperty("created_at") long createdAt,
    @JsonProperty("is_current") boolean isCurrent
) {}
