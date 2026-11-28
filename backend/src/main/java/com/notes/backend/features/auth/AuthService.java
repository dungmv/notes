package com.notes.backend.features.auth;

import com.notes.backend.common.config.NotesProperties;
import com.notes.backend.common.exception.DomainException;
import com.notes.backend.common.security.JwtProvider;
import com.notes.backend.features.auth.dto.*;
import com.notes.backend.features.auth.model.UserRecord;
import com.notes.backend.features.devices.DeviceService;
import java.nio.charset.StandardCharsets;
import java.security.InvalidKeyException;
import java.security.NoSuchAlgorithmException;
import java.time.Instant;
import java.util.Base64;
import java.util.HexFormat;
import java.util.Locale;
import java.util.UUID;
import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import org.springframework.http.HttpStatus;
import org.springframework.security.crypto.password.PasswordEncoder;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;
import tools.jackson.databind.ObjectMapper;

@Service
public class AuthService {
    private static final SaltResponse.KdfParamsDto DEFAULT_KDF =
        new SaltResponse.KdfParamsDto("argon2id", 65536, 3, 1);

    private final UserRepository userRepository;
    private final DeviceService deviceService;
    private final PasswordEncoder passwordEncoder;
    private final JwtProvider jwtProvider;
    private final ObjectMapper objectMapper;
    private final byte[] serverPepper;

    public AuthService(
        UserRepository userRepository,
        DeviceService deviceService,
        PasswordEncoder passwordEncoder,
        JwtProvider jwtProvider,
        ObjectMapper objectMapper,
        NotesProperties properties
    ) {
        this.userRepository = userRepository;
        this.deviceService = deviceService;
        this.passwordEncoder = passwordEncoder;
        this.jwtProvider = jwtProvider;
        this.objectMapper = objectMapper;
        this.serverPepper = properties.security().serverPepper().getBytes(StandardCharsets.UTF_8);
    }

    public SaltResponse getSalt(SaltRequest req) {
        String emailHash = hashEmail(req.email());
        return userRepository.findByEmailHash(emailHash)
            .map(u -> new SaltResponse(
                Base64.getEncoder().encodeToString(u.kdfSalt()),
                parseKdfParams(u.kdfParamsJson())
            ))
            .orElseGet(() -> new SaltResponse(
                Base64.getEncoder().encodeToString(hmacSha256("salt:" + req.email().trim().toLowerCase(Locale.ROOT))),
                DEFAULT_KDF
            ));
    }

    @Transactional
    public AuthResponse register(RegisterRequest req) {
        String emailHash = hashEmail(req.email());
        if (userRepository.findByEmailHash(emailHash).isPresent()) {
            throw new DomainException(HttpStatus.CONFLICT, "email_exists", "Email đã được đăng ký");
        }
        UUID userId = UUID.randomUUID();
        long now = Instant.now().toEpochMilli();
        byte[] kdfSalt = decodeB64(req.kdfSaltB64());
        byte[] wrappedAmk = decodeB64(req.wrappedAmkB64());
        byte[] recoveryWrapped = req.recoveryWrappedAmkB64() == null ? null : decodeB64(req.recoveryWrappedAmkB64());
        String recoveryVerifier = req.recoveryAuthKeyB64() == null ? null : passwordEncoder.encode(req.recoveryAuthKeyB64());

        UserRecord user = new UserRecord(
            userId,
            emailHash,
            passwordEncoder.encode(req.authKeyB64()),
            kdfSalt,
            serializeKdfParams(req.kdfParams()),
            wrappedAmk,
            recoveryWrapped,
            recoveryVerifier,
            null,
            1,
            0L,
            now
        );
        userRepository.insert(user);
        return issueSession(user, req.deviceId(), req.deviceName(), req.platform());
    }

    @Transactional
    public AuthResponse login(LoginRequest req) {
        UserRecord user = userRepository.findByEmailHash(hashEmail(req.email()))
            .orElseThrow(() -> new DomainException(
                HttpStatus.UNAUTHORIZED, "invalid_credentials", "Thông tin đăng nhập không đúng"
            ));
        if (!passwordEncoder.matches(req.authKeyB64(), user.authVerifier())) {
            throw new DomainException(
                HttpStatus.UNAUTHORIZED, "invalid_credentials", "Thông tin đăng nhập không đúng"
            );
        }
        return issueSession(user, req.deviceId(), req.deviceName(), req.platform());
    }

    public AuthResponse refresh(RefreshRequest req) {
        var rotated = deviceService.rotateRefreshToken(req.refreshToken(), req.deviceId());
        UserRecord user = userRepository.findById(rotated.userId())
            .orElseThrow(() -> new DomainException(
                HttpStatus.UNAUTHORIZED, "user_not_found", "Tài khoản không tồn tại"
            ));
        String accessToken = jwtProvider.createAccessToken(user.id(), rotated.deviceId());
        return new AuthResponse(user.id(), rotated.deviceId(), accessToken, rotated.refreshToken(), toKeyBundle(user));
    }

    @Transactional
    public void changeCredentials(UUID userId, UUID currentDeviceId, ChangeCredentialsRequest req) {
        UserRecord user = userRepository.findById(userId)
            .orElseThrow(() -> new DomainException(HttpStatus.NOT_FOUND, "user_not_found", "Không tìm thấy tài khoản"));
        if (!passwordEncoder.matches(req.currentAuthKeyB64(), user.authVerifier())) {
            throw new DomainException(HttpStatus.UNAUTHORIZED, "invalid_credentials", "Mật khẩu hiện tại không đúng");
        }
        userRepository.updateCredentials(
            userId,
            passwordEncoder.encode(req.newAuthKeyB64()),
            decodeB64(req.newKdfSaltB64()),
            serializeKdfParams(req.newKdfParams()),
            decodeB64(req.newWrappedAmkB64())
        );
        deviceService.revokeOtherDevicesRefreshTokens(userId, currentDeviceId);
    }

    public RecoverBundleResponse getRecoverBundle(RecoverBundleRequest req) {
        UserRecord user = verifyRecovery(req.email(), req.recoveryAuthKeyB64());
        return new RecoverBundleResponse(
            user.id(),
            Base64.getEncoder().encodeToString(user.recoveryWrappedAmk()),
            user.keyVersion()
        );
    }

    @Transactional
    public AuthResponse recoverAccount(RecoverRequest req) {
        UserRecord user = verifyRecovery(req.email(), req.recoveryAuthKeyB64());
        userRepository.updateCredentials(
            user.id(),
            passwordEncoder.encode(req.newAuthKeyB64()),
            decodeB64(req.newKdfSaltB64()),
            serializeKdfParams(req.newKdfParams()),
            decodeB64(req.newWrappedAmkB64())
        );
        deviceService.revokeOtherDevicesRefreshTokens(user.id(), req.deviceId());
        UserRecord updated = userRepository.findById(user.id()).orElseThrow();
        return issueSession(updated, req.deviceId(), req.deviceName(), req.platform());
    }

    public KeyBundleResponse getKeyBundle(UUID userId) {
        UserRecord user = userRepository.findById(userId)
            .orElseThrow(() -> new DomainException(HttpStatus.NOT_FOUND, "user_not_found", "Không tìm thấy tài khoản"));
        return toKeyBundle(user);
    }

    @Transactional
    public KeyBundleResponse rotateKey(UUID userId, RotateKeyRequest req) {
        int updated = userRepository.rotateKey(
            userId,
            req.expectedKeyVersion(),
            decodeB64(req.newWrappedAmkB64()),
            req.recoveryWrappedAmkB64() == null ? null : decodeB64(req.recoveryWrappedAmkB64()),
            decodeB64(req.keyRingEncryptedB64())
        );
        if (updated == 0) {
            throw new DomainException(HttpStatus.CONFLICT, "key_version_conflict", "Phiên bản khóa không khớp");
        }
        return getKeyBundle(userId);
    }

    private UserRecord verifyRecovery(String email, String recoveryAuthKeyB64) {
        UserRecord user = userRepository.findByEmailHash(hashEmail(email))
            .orElseThrow(() -> new DomainException(
                HttpStatus.UNAUTHORIZED, "invalid_recovery", "Thông tin phục hồi không hợp lệ"
            ));
        if (user.recoveryAuthVerifier() == null || user.recoveryWrappedAmk() == null
            || !passwordEncoder.matches(recoveryAuthKeyB64, user.recoveryAuthVerifier())) {
            throw new DomainException(
                HttpStatus.UNAUTHORIZED, "invalid_recovery", "Thông tin phục hồi không hợp lệ"
            );
        }
        return user;
    }

    private AuthResponse issueSession(UserRecord user, UUID deviceId, String deviceName, String platform) {
        String refreshToken = deviceService.registerDeviceAndIssueRefreshToken(
            user.id(), deviceId, deviceName, platform
        );
        String accessToken = jwtProvider.createAccessToken(user.id(), deviceId);
        return new AuthResponse(user.id(), deviceId, accessToken, refreshToken, toKeyBundle(user));
    }

    private static KeyBundleResponse toKeyBundle(UserRecord u) {
        return new KeyBundleResponse(
            Base64.getEncoder().encodeToString(u.wrappedAmk()),
            u.recoveryWrappedAmk() == null ? null : Base64.getEncoder().encodeToString(u.recoveryWrappedAmk()),
            u.keyRingEncrypted() == null ? null : Base64.getEncoder().encodeToString(u.keyRingEncrypted()),
            u.keyVersion()
        );
    }

    private String hashEmail(String email) {
        return HexFormat.of().formatHex(hmacSha256("email:" + email.trim().toLowerCase(Locale.ROOT)));
    }

    private byte[] hmacSha256(String value) {
        try {
            Mac mac = Mac.getInstance("HmacSHA256");
            mac.init(new SecretKeySpec(serverPepper, "HmacSHA256"));
            return mac.doFinal(value.getBytes(StandardCharsets.UTF_8));
        } catch (NoSuchAlgorithmException | InvalidKeyException ex) {
            throw new IllegalStateException(ex);
        }
    }

    private static byte[] decodeB64(String b64) {
        try {
            return Base64.getDecoder().decode(b64);
        } catch (IllegalArgumentException ex) {
            throw new DomainException(HttpStatus.BAD_REQUEST, "invalid_base64", "Chuỗi Base64 không hợp lệ");
        }
    }

    private String serializeKdfParams(SaltResponse.KdfParamsDto dto) {
        try {
            return objectMapper.writeValueAsString(dto);
        } catch (Exception ex) {
            throw new DomainException(HttpStatus.BAD_REQUEST, "invalid_kdf_params", "Tham số KDF không hợp lệ");
        }
    }

    private SaltResponse.KdfParamsDto parseKdfParams(String json) {
        try {
            return objectMapper.readValue(json, SaltResponse.KdfParamsDto.class);
        } catch (Exception ex) {
            return DEFAULT_KDF;
        }
    }
}
