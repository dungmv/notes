package com.notes.backend.features.auth.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

public record KeyBundleResponse(
    @JsonProperty("wrapped_amk_b64") String wrappedAmkB64,
    @JsonProperty("recovery_wrapped_amk_b64") String recoveryWrappedAmkB64,
    @JsonProperty("key_ring_encrypted_b64") String keyRingEncryptedB64,
    @JsonProperty("key_version") int keyVersion
) {}
