package com.notes.backend.features.auth;

import com.notes.backend.common.security.AuthPrincipal;
import com.notes.backend.features.auth.dto.*;
import jakarta.validation.Valid;
import org.springframework.http.ResponseEntity;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/v1/auth")
public class AuthController {
    private final AuthService authService;

    public AuthController(AuthService authService) {
        this.authService = authService;
    }

    @PostMapping("/salt")
    public ResponseEntity<SaltResponse> salt(@Valid @RequestBody SaltRequest request) {
        return ResponseEntity.ok(authService.getSalt(request));
    }

    @PostMapping("/register")
    public ResponseEntity<AuthResponse> register(@Valid @RequestBody RegisterRequest request) {
        return ResponseEntity.ok(authService.register(request));
    }

    @PostMapping("/login")
    public ResponseEntity<AuthResponse> login(@Valid @RequestBody LoginRequest request) {
        return ResponseEntity.ok(authService.login(request));
    }

    @PostMapping("/refresh")
    public ResponseEntity<AuthResponse> refresh(@Valid @RequestBody RefreshRequest request) {
        return ResponseEntity.ok(authService.refresh(request));
    }

    @PutMapping("/credentials")
    public ResponseEntity<Void> changeCredentials(
        @AuthenticationPrincipal AuthPrincipal principal,
        @Valid @RequestBody ChangeCredentialsRequest request
    ) {
        authService.changeCredentials(principal.userId(), principal.deviceId(), request);
        return ResponseEntity.noContent().build();
    }

    @PostMapping("/recover/bundle")
    public ResponseEntity<RecoverBundleResponse> recoverBundle(@Valid @RequestBody RecoverBundleRequest request) {
        return ResponseEntity.ok(authService.getRecoverBundle(request));
    }

    @PostMapping("/recover")
    public ResponseEntity<AuthResponse> recover(@Valid @RequestBody RecoverRequest request) {
        return ResponseEntity.ok(authService.recoverAccount(request));
    }
}
