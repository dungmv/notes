package com.notes.backend.features.blob.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.NotBlank;

public record CompleteUploadRequest(
    @NotBlank @JsonProperty("upload_token") String uploadToken
) {}
