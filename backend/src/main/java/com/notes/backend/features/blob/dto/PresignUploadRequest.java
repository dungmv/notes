package com.notes.backend.features.blob.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotBlank;

public record PresignUploadRequest(
    @NotBlank @JsonProperty("content_hash") String contentHash,
    @Min(1) @JsonProperty("byte_size") long byteSize,
    @NotBlank @JsonProperty("ciphertext_sha256") String ciphertextSha256
) {}
