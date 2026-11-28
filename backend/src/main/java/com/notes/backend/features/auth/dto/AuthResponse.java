package com.notes.backend.features.auth.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.UUID;

public record AuthResponse(
    @JsonProperty("user_id") UUID userId,
    @JsonProperty("device_id") UUID deviceId,
    @JsonProperty("access_token") String accessToken,
    @JsonProperty("refresh_token") String refreshToken,
    @JsonProperty("key_bundle") KeyBundleResponse keyBundle
) {}
