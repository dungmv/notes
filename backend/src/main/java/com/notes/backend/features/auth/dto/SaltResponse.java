package com.notes.backend.features.auth.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

public record SaltResponse(
    @JsonProperty("kdf_salt_b64") String kdfSaltB64,
    @JsonProperty("kdf_params") KdfParamsDto kdfParams
) {
    public record KdfParamsDto(String alg, @JsonProperty("m_kib") int mKib, int t, int p) {}
}
