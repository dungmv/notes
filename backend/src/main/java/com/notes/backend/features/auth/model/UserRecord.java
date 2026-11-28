package com.notes.backend.features.auth.model;

import java.util.UUID;

public record UserRecord(
    UUID id,
    String emailHash,
    String authVerifier,
    byte[] kdfSalt,
    String kdfParamsJson,
    byte[] wrappedAmk,
    byte[] recoveryWrappedAmk,
    String recoveryAuthVerifier,
    byte[] keyRingEncrypted,
    int keyVersion,
    long lastSyncSeq,
    long createdAt
) {}
