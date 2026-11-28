package com.notes.backend.features.sync.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.List;
import java.util.UUID;

public record PullSyncResponse(
    List<PulledDeltaDto> deltas,
    @JsonProperty("next_cursor") long nextCursor,
    @JsonProperty("has_more") boolean hasMore
) {
    public record PulledDeltaDto(
        @JsonProperty("server_seq") long serverSeq,
        @JsonProperty("device_id") UUID deviceId,
        @JsonProperty("entity_type") String entityType,
        @JsonProperty("entity_id") UUID entityId,
        @JsonProperty("client_op_id") UUID clientOpId,
        @JsonProperty("crypto_version") short cryptoVersion,
        @JsonProperty("key_version") int keyVersion,
        @JsonProperty("encrypted_payload_b64") String encryptedPayloadB64,
        @JsonProperty("created_at") long createdAt
    ) {}
}
