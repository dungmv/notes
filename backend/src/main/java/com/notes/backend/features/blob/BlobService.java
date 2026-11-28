package com.notes.backend.features.blob;

import com.notes.backend.common.config.NotesProperties;
import com.notes.backend.common.exception.DomainException;
import com.notes.backend.features.blob.dto.CompleteUploadRequest;
import com.notes.backend.features.blob.dto.PresignDownloadResponse;
import com.notes.backend.features.blob.dto.PresignUploadRequest;
import com.notes.backend.features.blob.dto.PresignUploadResponse;
import java.time.Duration;
import java.time.Instant;
import java.util.UUID;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;
import software.amazon.awssdk.services.s3.S3Client;
import software.amazon.awssdk.services.s3.model.CopyObjectRequest;
import software.amazon.awssdk.services.s3.model.DeleteObjectRequest;
import software.amazon.awssdk.services.s3.model.GetObjectRequest;
import software.amazon.awssdk.services.s3.model.HeadObjectRequest;
import software.amazon.awssdk.services.s3.model.PutObjectRequest;
import software.amazon.awssdk.services.s3.model.S3Exception;
import software.amazon.awssdk.services.s3.presigner.S3Presigner;
import software.amazon.awssdk.services.s3.presigner.model.GetObjectPresignRequest;
import software.amazon.awssdk.services.s3.presigner.model.PutObjectPresignRequest;

@Service
public class BlobService {
    private final BlobRepository blobRepository;
    private final S3Client s3Client;
    private final S3Presigner s3Presigner;
    private final String bucket;
    private final Duration presignTtl;

    public BlobService(
        BlobRepository blobRepository,
        S3Client s3Client,
        S3Presigner s3Presigner,
        NotesProperties properties
    ) {
        this.blobRepository = blobRepository;
        this.s3Client = s3Client;
        this.s3Presigner = s3Presigner;
        this.bucket = properties.s3().bucket();
        this.presignTtl = Duration.ofSeconds(properties.s3().presignTtlSeconds());
    }

    @Transactional
    public PresignUploadResponse presignUpload(UUID userId, PresignUploadRequest req) {
        var existing = blobRepository.findUserBlob(userId, req.contentHash());
        if (existing.isPresent()) {
            var row = existing.get();
            if (row.byteSize() == req.byteSize() && row.ciphertextSha256().equals(req.ciphertextSha256())) {
                return new PresignUploadResponse(true, null, null);
            }
            throw new DomainException(
                HttpStatus.CONFLICT,
                "blob_conflict",
                "Blob đã tồn tại với kích thước hoặc checksum khác"
            );
        }

        String token = UUID.randomUUID().toString();
        String stagingKey = "staging/" + userId + "/" + token;
        String finalKey = "blobs/" + userId + "/" + req.contentHash();
        long expiresAt = Instant.now().plus(presignTtl).toEpochMilli();

        blobRepository.insertUploadToken(
            token,
            userId,
            req.contentHash(),
            req.byteSize(),
            req.ciphertextSha256(),
            stagingKey,
            finalKey,
            expiresAt
        );

        var putReq = PutObjectRequest.builder()
            .bucket(bucket)
            .key(stagingKey)
            .contentLength(req.byteSize())
            .build();
        var presigned = s3Presigner.presignPutObject(PutObjectPresignRequest.builder()
            .signatureDuration(presignTtl)
            .putObjectRequest(putReq)
            .build());
        return new PresignUploadResponse(false, presigned.url().toString(), token);
    }

    @Transactional
    public void completeUpload(UUID userId, CompleteUploadRequest req) {
        var tokenRow = blobRepository.findUploadToken(req.uploadToken(), userId)
            .orElseThrow(() -> new DomainException(
                HttpStatus.NOT_FOUND, "upload_token_not_found", "Upload token không tồn tại"
            ));
        if (tokenRow.completedAt() != null) {
            return;
        }
        long now = Instant.now().toEpochMilli();
        if (tokenRow.expiresAt() < now) {
            throw new DomainException(HttpStatus.GONE, "upload_token_expired", "Upload token đã hết hạn");
        }

        var existing = blobRepository.findUserBlob(userId, tokenRow.contentHash());
        if (existing.isPresent()) {
            var row = existing.get();
            if (row.byteSize() != tokenRow.byteSize()
                || !row.ciphertextSha256().equals(tokenRow.ciphertextSha256())) {
                throw new DomainException(HttpStatus.CONFLICT, "blob_conflict", "Blob xung đột checksum");
            }
            blobRepository.markTokenCompleted(req.uploadToken(), now);
            return;
        }

        try {
            var head = s3Client.headObject(HeadObjectRequest.builder()
                .bucket(bucket)
                .key(tokenRow.stagingKey())
                .build());
            if (!Long.valueOf(tokenRow.byteSize()).equals(head.contentLength())) {
                throw new DomainException(
                    HttpStatus.BAD_REQUEST, "blob_size_mismatch", "Kích thước blob tải lên không khớp"
                );
            }
            s3Client.copyObject(CopyObjectRequest.builder()
                .sourceBucket(bucket)
                .sourceKey(tokenRow.stagingKey())
                .destinationBucket(bucket)
                .destinationKey(tokenRow.finalKey())
                .build());
            s3Client.deleteObject(DeleteObjectRequest.builder()
                .bucket(bucket)
                .key(tokenRow.stagingKey())
                .build());
        } catch (S3Exception ex) {
            throw new DomainException(
                HttpStatus.BAD_REQUEST, "blob_not_uploaded", "Không tìm thấy blob đã upload trên storage"
            );
        }

        blobRepository.insertUserBlob(
            userId,
            tokenRow.contentHash(),
            tokenRow.byteSize(),
            tokenRow.finalKey(),
            tokenRow.ciphertextSha256(),
            now
        );
        blobRepository.markTokenCompleted(req.uploadToken(), now);
    }

    public PresignDownloadResponse presignDownload(UUID userId, String contentHash) {
        var row = blobRepository.findUserBlob(userId, contentHash)
            .orElseThrow(() -> new DomainException(HttpStatus.NOT_FOUND, "blob_not_found", "Không tìm thấy blob"));
        var getReq = GetObjectRequest.builder()
            .bucket(bucket)
            .key(row.storageKey())
            .build();
        var presigned = s3Presigner.presignGetObject(GetObjectPresignRequest.builder()
            .signatureDuration(presignTtl)
            .getObjectRequest(getReq)
            .build());
        return new PresignDownloadResponse(presigned.url().toString());
    }
}
