package com.notes.backend.features.blob;

import com.notes.backend.common.security.AuthPrincipal;
import com.notes.backend.features.blob.dto.CompleteUploadRequest;
import com.notes.backend.features.blob.dto.PresignDownloadResponse;
import com.notes.backend.features.blob.dto.PresignUploadRequest;
import com.notes.backend.features.blob.dto.PresignUploadResponse;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.ResponseEntity;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/v1/blobs")
public class BlobController {
    private final BlobService blobService;

    public BlobController(BlobService blobService) {
        this.blobService = blobService;
    }

    @PostMapping("/presign-upload")
    public ResponseEntity<PresignUploadResponse> presignUpload(
        @AuthenticationPrincipal AuthPrincipal principal,
        @Valid @RequestBody PresignUploadRequest request
    ) {
        return ResponseEntity.ok(blobService.presignUpload(principal.userId(), request));
    }

    @PostMapping("/complete-upload")
    public ResponseEntity<Void> completeUpload(
        @AuthenticationPrincipal AuthPrincipal principal,
        @Valid @RequestBody CompleteUploadRequest request
    ) {
        blobService.completeUpload(principal.userId(), request);
        return ResponseEntity.noContent().build();
    }

    @GetMapping("/presign-download")
    public ResponseEntity<PresignDownloadResponse> presignDownload(
        @AuthenticationPrincipal AuthPrincipal principal,
        @RequestParam("hash") @NotBlank String contentHash
    ) {
        return ResponseEntity.ok(blobService.presignDownload(principal.userId(), contentHash));
    }
}
