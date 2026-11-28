package com.notes.backend.features.sync;

import com.notes.backend.common.security.AuthPrincipal;
import com.notes.backend.features.sync.dto.PullSyncResponse;
import com.notes.backend.features.sync.dto.PushSyncRequest;
import com.notes.backend.features.sync.dto.PushSyncResponse;
import jakarta.validation.Valid;
import org.springframework.http.ResponseEntity;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/v1/sync")
public class SyncController {
    private final SyncService syncService;

    public SyncController(SyncService syncService) {
        this.syncService = syncService;
    }

    @PostMapping("/push")
    public ResponseEntity<PushSyncResponse> push(
        @AuthenticationPrincipal AuthPrincipal principal,
        @Valid @RequestBody PushSyncRequest request
    ) {
        return ResponseEntity.ok(syncService.push(principal.userId(), principal.deviceId(), request));
    }

    @GetMapping("/pull")
    public ResponseEntity<PullSyncResponse> pull(
        @AuthenticationPrincipal AuthPrincipal principal,
        @RequestParam(name = "since_seq", defaultValue = "0") long sinceSeq,
        @RequestParam(name = "limit", defaultValue = "200") int limit
    ) {
        return ResponseEntity.ok(syncService.pull(principal.userId(), sinceSeq, limit));
    }
}
