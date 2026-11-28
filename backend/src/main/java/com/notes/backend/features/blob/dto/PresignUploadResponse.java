package com.notes.backend.features.blob.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

public record PresignUploadResponse(
    boolean exists,
    @JsonProperty("upload_url") String uploadUrl,
    @JsonProperty("upload_token") String uploadToken
) {}
