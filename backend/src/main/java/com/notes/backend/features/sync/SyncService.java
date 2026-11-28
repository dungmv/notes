package com.notes.backend.features.sync;

import com.notes.backend.common.exception.DomainException;
import com.notes.backend.features.sync.dto.PullSyncResponse;
import com.notes.backend.features.sync.dto.PushSyncRequest;
import com.notes.backend.features.sync.dto.PushSyncResponse;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.UUID;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

@Service
public class SyncService {
    private final SyncRepository syncRepository;

    public SyncService(SyncRepository syncRepository) {
        this.syncRepository = syncRepository;
    }

    @Transactional
    public PushSyncResponse push(UUID userId, UUID deviceId, PushSyncRequest request) {
        Set<UUID> seenOpIds = new HashSet<>();
        for (var item : request.deltas()) {
            if (!seenOpIds.add(item.clientOpId())) {
                throw new DomainException(
                    HttpStatus.BAD_REQUEST,
                    "duplicate_client_op_id",
                    "Trùng lặp client_op_id trong cùng batch"
                );
            }
        }

        long currentSeq = syncRepository.lockUserLastSyncSeq(userId);
        long initialSeq = currentSeq;
        long now = Instant.now().toEpochMilli();
        List<PushSyncResponse.Receipt> receipts = new ArrayList<>(request.deltas().size());

        for (var item : request.deltas()) {
            byte[] payload = decodeB64(item.encryptedPayloadB64());
            var existing = syncRepository.findByClientOpId(userId, item.clientOpId());
            if (existing.isPresent()) {
                var row = existing.get();
                boolean same = row.entityType().equals(item.entityType())
                    && row.entityId().equals(item.entityId())
                    && row.cryptoVersion() == item.cryptoVersion()
                    && row.keyVersion() == item.keyVersion()
                    && Arrays.equals(row.encryptedPayload(), payload);
                if (!same) {
                    throw new DomainException(
                        HttpStatus.CONFLICT,
                        "client_op_conflict",
                        "client_op_id đã tồn tại với nội dung khác"
                    );
                }
                receipts.add(new PushSyncResponse.Receipt(item.clientOpId(), row.serverSeq()));
            } else {
                currentSeq += 1;
                syncRepository.insertDelta(
                    currentSeq,
                    userId,
                    deviceId,
                    item.entityType(),
                    item.entityId(),
                    item.clientOpId(),
                    item.cryptoVersion(),
                    item.keyVersion(),
                    payload,
                    now
                );
                receipts.add(new PushSyncResponse.Receipt(item.clientOpId(), currentSeq));
            }
        }

        if (currentSeq != initialSeq) {
            syncRepository.updateLastSyncSeq(userId, currentSeq);
        }
        return new PushSyncResponse(receipts);
    }

    public PullSyncResponse pull(UUID userId, long sinceSeq, int limit) {
        int clamped = Math.max(1, Math.min(limit, 500));
        var rows = syncRepository.pullSince(userId, sinceSeq, clamped + 1);
        boolean hasMore = rows.size() > clamped;
        var page = hasMore ? rows.subList(0, clamped) : rows;
        long nextCursor = page.isEmpty() ? sinceSeq : page.getLast().serverSeq();

        var deltas = page.stream()
            .map(r -> new PullSyncResponse.PulledDeltaDto(
                r.serverSeq(),
                r.deviceId(),
                r.entityType(),
                r.entityId(),
                r.clientOpId(),
                r.cryptoVersion(),
                r.keyVersion(),
                Base64.getEncoder().encodeToString(r.encryptedPayload()),
                r.createdAt()
            ))
            .toList();
        return new PullSyncResponse(deltas, nextCursor, hasMore);
    }

    private static byte[] decodeB64(String b64) {
        try {
            return Base64.getDecoder().decode(b64);
        } catch (IllegalArgumentException ex) {
            throw new DomainException(HttpStatus.BAD_REQUEST, "invalid_base64", "Payload Base64 không hợp lệ");
        }
    }
}
