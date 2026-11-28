package com.notes.backend.features.sync.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.List;
import java.util.UUID;

public record PushSyncResponse(List<Receipt> receipts) {
    public record Receipt(
        @JsonProperty("client_op_id") UUID clientOpId,
        @JsonProperty("server_seq") long serverSeq
    ) {}
}
