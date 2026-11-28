package com.notes.backend.features.auth.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotBlank;

public record RotateKeyRequest(
    @Min(1) @JsonProperty("expected_key_version") int expectedKeyVersion,
    @NotBlank @JsonProperty("new_wrapped_amk_b64") String newWrappedAmkB64,
    @JsonProperty("recovery_wrapped_amk_b64") String recoveryWrappedAmkB64,
    @NotBlank @JsonProperty("key_ring_encrypted_b64") String keyRingEncryptedB64
) {}
