package com.notes.backend.features.auth.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.Email;
import jakarta.validation.constraints.NotBlank;

public record RecoverBundleRequest(
    @NotBlank @Email String email,
    @NotBlank @JsonProperty("recovery_auth_key_b64") String recoveryAuthKeyB64
) {}
