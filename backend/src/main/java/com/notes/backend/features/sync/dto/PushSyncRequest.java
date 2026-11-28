package com.notes.backend.features.sync.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.Valid;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotEmpty;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;
import java.util.List;
import java.util.UUID;

public record PushSyncRequest(
    @NotEmpty @Size(max = 500) List<@Valid SyncDeltaItem> deltas
) {
    public record SyncDeltaItem(
        @NotNull @JsonProperty("client_op_id") UUID clientOpId,
        @NotBlank @JsonProperty("entity_type") String entityType,
        @NotNull @JsonProperty("entity_id") UUID entityId,
        @Min(1) @JsonProperty("crypto_version") short cryptoVersion,
        @Min(1) @JsonProperty("key_version") int keyVersion,
        @NotBlank @JsonProperty("encrypted_payload_b64") String encryptedPayloadB64
    ) {}
}
