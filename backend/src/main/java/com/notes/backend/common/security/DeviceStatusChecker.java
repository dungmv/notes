package com.notes.backend.common.security;

import java.util.UUID;

public interface DeviceStatusChecker {
    boolean isActiveDevice(UUID userId, UUID deviceId);
}
