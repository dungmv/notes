package com.notes.backend.features.auth;

import com.notes.backend.common.security.AuthPrincipal;
import com.notes.backend.features.auth.dto.KeyBundleResponse;
import com.notes.backend.features.auth.dto.RotateKeyRequest;
import jakarta.validation.Valid;
import org.springframework.http.ResponseEntity;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/v1/keys")
public class KeyController {
    private final AuthService authService;

    public KeyController(AuthService authService) {
        this.authService = authService;
    }

    @GetMapping("/bundle")
    public ResponseEntity<KeyBundleResponse> getBundle(@AuthenticationPrincipal AuthPrincipal principal) {
        return ResponseEntity.ok(authService.getKeyBundle(principal.userId()));
    }

    @PutMapping("/rotate")
    public ResponseEntity<KeyBundleResponse> rotate(
        @AuthenticationPrincipal AuthPrincipal principal,
        @Valid @RequestBody RotateKeyRequest request
    ) {
        return ResponseEntity.ok(authService.rotateKey(principal.userId(), request));
    }
}
