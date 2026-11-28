package com.notes.backend.common.security;

import com.notes.backend.common.config.NotesProperties;
import io.jsonwebtoken.Claims;
import io.jsonwebtoken.JwtException;
import io.jsonwebtoken.Jwts;
import io.jsonwebtoken.security.Keys;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.Instant;
import java.util.Date;
import java.util.UUID;
import javax.crypto.SecretKey;
import org.springframework.stereotype.Component;

@Component
public class JwtProvider {
    private final SecretKey key;
    private final long accessTokenTtlSeconds;

    public JwtProvider(NotesProperties properties) {
        byte[] raw = properties.security().jwtSecret().getBytes(StandardCharsets.UTF_8);
        this.key = Keys.hmacShaKeyFor(sha256(raw));
        this.accessTokenTtlSeconds = properties.security().accessTokenTtlSeconds();
    }

    public String createAccessToken(UUID userId, UUID deviceId) {
        Instant now = Instant.now();
        return Jwts.builder()
            .subject(userId.toString())
            .claim("device_id", deviceId.toString())
            .issuedAt(Date.from(now))
            .expiration(Date.from(now.plusSeconds(accessTokenTtlSeconds)))
            .signWith(key)
            .compact();
    }

    public AuthPrincipal parseAccessToken(String token) {
        try {
            Claims claims = Jwts.parser()
                .verifyWith(key)
                .build()
                .parseSignedClaims(token)
                .getPayload();
            UUID userId = UUID.fromString(claims.getSubject());
            UUID deviceId = UUID.fromString(claims.get("device_id", String.class));
            return new AuthPrincipal(userId, deviceId);
        } catch (JwtException | IllegalArgumentException ex) {
            return null;
        }
    }

    private static byte[] sha256(byte[] input) {
        try {
            return MessageDigest.getInstance("SHA-256").digest(input);
        } catch (NoSuchAlgorithmException ex) {
            throw new IllegalStateException(ex);
        }
    }
}
