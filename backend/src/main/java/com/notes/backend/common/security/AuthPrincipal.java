package com.notes.backend.common.security;

import java.util.UUID;

public record AuthPrincipal(UUID userId, UUID deviceId) {}
