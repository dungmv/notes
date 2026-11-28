package com.notes.backend.features.auth.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import java.util.UUID;

public record RefreshRequest(
    @NotBlank @JsonProperty("refresh_token") String refreshToken,
    @NotNull @JsonProperty("device_id") UUID deviceId
) {}
