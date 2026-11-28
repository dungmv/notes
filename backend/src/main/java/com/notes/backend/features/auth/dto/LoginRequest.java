package com.notes.backend.features.auth.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.Email;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import java.util.UUID;

public record LoginRequest(
    @NotBlank @Email String email,
    @NotBlank @JsonProperty("auth_key_b64") String authKeyB64,
    @NotNull @JsonProperty("device_id") UUID deviceId,
    @NotBlank @JsonProperty("device_name") String deviceName,
    @NotBlank String platform
) {}
