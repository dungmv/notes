package com.notes.backend.common.config;

import org.springframework.boot.context.properties.ConfigurationProperties;

@ConfigurationProperties(prefix = "notes")
public record NotesProperties(
    SecurityProperties security,
    S3Properties s3
) {
    public record SecurityProperties(
        String jwtSecret,
        String serverPepper,
        long accessTokenTtlSeconds,
        long refreshTokenTtlSeconds
    ) {}

    public record S3Properties(
        String endpoint,
        String region,
        String bucket,
        String accessKey,
        String secretKey,
        boolean pathStyleAccess,
        long presignTtlSeconds
    ) {}
}
