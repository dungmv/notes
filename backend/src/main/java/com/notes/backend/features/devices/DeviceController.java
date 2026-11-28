package com.notes.backend.features.devices;

import com.notes.backend.common.security.AuthPrincipal;
import com.notes.backend.features.devices.dto.DeviceListResponse;
import java.util.UUID;
import org.springframework.http.ResponseEntity;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/v1/devices")
public class DeviceController {
    private final DeviceService deviceService;

    public DeviceController(DeviceService deviceService) {
        this.deviceService = deviceService;
    }

    @GetMapping
    public ResponseEntity<DeviceListResponse> list(@AuthenticationPrincipal AuthPrincipal principal) {
        return ResponseEntity.ok(deviceService.listDevices(principal.userId(), principal.deviceId()));
    }

    @DeleteMapping("/{id}")
    public ResponseEntity<Void> revoke(
        @AuthenticationPrincipal AuthPrincipal principal,
        @PathVariable("id") UUID deviceId
    ) {
        deviceService.revokeDevice(principal.userId(), deviceId);
        return ResponseEntity.noContent().build();
    }
}
