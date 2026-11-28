package com.notes.backend.features.blob.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

public record PresignDownloadResponse(
    @JsonProperty("download_url") String downloadUrl
) {}
