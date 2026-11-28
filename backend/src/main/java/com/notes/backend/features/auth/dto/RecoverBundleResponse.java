package com.notes.backend.features.auth.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.UUID;

public record RecoverBundleResponse(
    @JsonProperty("user_id") UUID userId,
    @JsonProperty("recovery_wrapped_amk_b64") String recoveryWrappedAmkB64,
    @JsonProperty("key_version") int keyVersion
) {}
