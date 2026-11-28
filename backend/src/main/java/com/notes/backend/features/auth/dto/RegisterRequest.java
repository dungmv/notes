package com.notes.backend.features.auth.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.Valid;
import jakarta.validation.constraints.Email;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import java.util.UUID;

public record RegisterRequest(
    @NotBlank @Email String email,
    @NotBlank @JsonProperty("auth_key_b64") String authKeyB64,
    @NotBlank @JsonProperty("kdf_salt_b64") String kdfSaltB64,
    @NotNull @Valid @JsonProperty("kdf_params") SaltResponse.KdfParamsDto kdfParams,
    @NotBlank @JsonProperty("wrapped_amk_b64") String wrappedAmkB64,
    @JsonProperty("recovery_wrapped_amk_b64") String recoveryWrappedAmkB64,
    @JsonProperty("recovery_auth_key_b64") String recoveryAuthKeyB64,
    @NotNull @JsonProperty("device_id") UUID deviceId,
    @NotBlank @JsonProperty("device_name") String deviceName,
    @NotBlank String platform
) {}
