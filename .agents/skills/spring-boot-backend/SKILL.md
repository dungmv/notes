---
name: spring-boot-backend
description: >-
  Conventions and workflow for developing the Java 21+ / Spring Boot 4.x backend.
  Use whenever creating or modifying backend features, REST controllers, DTOs,
  services, repositories, or Flyway database migrations. Enforces NestJS-style
  feature-based packaging, strict typed Java record DTOs (no Map bodies), and
  immutable Flyway migrations.
---

# Spring Boot 4 Backend Development Skill

Follow these architectural and coding rules for all work in `backend/` (Java 21+, Spring Boot 4.x, PostgreSQL, S3/MinIO, Centrifugo).

---

## 1. Feature-Based Package Organization (NestJS-Style)

Organize code by **business feature**, never by global technical layers (`controllers/`, `services/`, `repositories/`).

```text
src/main/java/com/notes/backend/
├── common/                         # Cross-cutting infrastructure only
│   ├── config/                     # SecurityConfig, S3Config, CentrifugoConfig
│   ├── exception/                  # GlobalExceptionHandler (ProblemDetail), DomainException
│   └── security/                   # JwtProvider, CurrentUser, AuthPrincipal
└── features/
    ├── auth/
    │   ├── AuthController.java
    │   ├── AuthService.java
    │   ├── UserRepository.java
    │   ├── dto/
    │   │   ├── LoginRequest.java
    │   │   └── AuthResponse.java
    │   └── model/
    │       └── User.java
    ├── sync/
    │   ├── SyncController.java
    │   ├── SyncService.java
    │   ├── SyncRepository.java
    │   ├── dto/
    │   └── model/
    ├── blob/
    │   ├── BlobController.java
    │   ├── BlobService.java
    │   ├── BlobRepository.java
    │   └── dto/
    └── realtime/
        ├── CentrifugoPublisher.java
        └── CentrifugoTokenService.java
```

### Module Boundaries
- **Thin Controllers**: `*Controller` only handles HTTP routing, `@Valid` DTO binding, auth principal extraction, and calling its own `*Service`.
- **No Cross-Feature Repository Access**: Feature A must never inject Feature B's `*Repository`. Communicate via Feature B's `*Service` or Spring `ApplicationEventPublisher`.
- **Constructor Injection Only**: Never use field injection (`@Autowired` on fields).

---

## 2. Strict Typed DTOs (Never Use `Map` for Request/Response Bodies)

- **Forbidden**: `Map<String, Object>`, `Map<?, ?>`, `JsonNode`, or raw `Object` in `@RequestBody` parameters or controller return types.
- **Required**: Define explicit, immutable **Java `record`** types inside `features/<feature>/dto/` for every request and response body.
- **Validation**:
  - Always annotate `@RequestBody` parameters with `@Valid`.
  - Use Jakarta Bean Validation annotations (`@NotNull`, `@NotBlank`, `@NotEmpty`, `@Size`, `@Min`, `@Max`) directly on `record` components.
  - Use `@JsonProperty` if JSON snake_case differs from Java camelCase (or configure Jackson `SNAKE_CASE` consistently).

```java
// GOOD
public record PushSyncRequest(
    @NotEmpty @Size(max = 500) List<@Valid SyncDeltaItem> deltas
) {}

@PostMapping("/push")
public ResponseEntity<PushSyncResponse> push(
    @AuthenticationPrincipal AuthPrincipal user,
    @RequestHeader("X-Device-Id") UUID deviceId,
    @Valid @RequestBody PushSyncRequest request
) {
    return ResponseEntity.ok(syncService.push(user.userId(), deviceId, request));
}
```

- **Error Responses**: Handle validation and domain errors centrally in `common/exception/GlobalExceptionHandler` (`@RestControllerAdvice`) returning Spring `ProblemDetail` (RFC 9457).

---

## 3. Database Migrations via Flyway

All PostgreSQL schema changes MUST go through **Flyway** in `src/main/resources/db/migration/`.

### Rules
1. **Naming Convention**: `V<version>__<snake_case_description>.sql` (double underscore `__`), e.g.:
   - `V1__init_schema.sql`
   - `V2__add_device_last_seen_index.sql`
2. **Immutability**: Never modify, rename, or delete a `V*` migration file after it has been committed/applied. Always add a new `V<next>__...sql` migration to alter tables or fix mistakes.
3. **No Auto-DDL**: Never rely on `ddl-auto: update` or `create`. Keep `spring.flyway.enabled=true` and `spring.jpa.hibernate.ddl-auto=validate` (or `none` when using JDBC/jOOQ).
4. **Idempotent & Safe SQL**: Prefer explicit constraints, indexes (`CREATE INDEX CONCURRENTLY` when appropriate outside transactions, or standard `CREATE INDEX` in initial setup), and `NOT NULL` defaults where safe.

---

## 4. Verification Checklist

Before completing any backend change:
1. Confirm no `Map` is used as a `@RequestBody` or controller response type.
2. Confirm all new endpoints, DTOs, and services reside under `features/<feature>/`.
3. If schema changed, confirm a new `V*__*.sql` Flyway migration was added (and no existing `V*` file was edited).
4. Run build & tests (including Flyway validation / Testcontainers if configured):
   - Maven: `./mvnw test` (or `mvnw.cmd test` on Windows)
   - Gradle: `./gradlew test` (or `gradlew.bat test` on Windows)
