package com.notes.backend.features.auth.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;

public record ChangeCredentialsRequest(
    @NotBlank @JsonProperty("current_auth_key_b64") String currentAuthKeyB64,
    @NotBlank @JsonProperty("new_auth_key_b64") String newAuthKeyB64,
    @NotBlank @JsonProperty("new_kdf_salt_b64") String newKdfSaltB64,
    @NotNull @Valid @JsonProperty("new_kdf_params") SaltResponse.KdfParamsDto newKdfParams,
    @NotBlank @JsonProperty("new_wrapped_amk_b64") String newWrappedAmkB64
) {}
