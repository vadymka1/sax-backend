# Architecture Specification & Mermaid Diagrams

## 1. Modular Layering Architecture

```mermaid
graph TD
    Client[React Admin / Public Web Client] --> |HTTP / JSON| API[Rocket 0.5.1 API Layer]
    
    subgraph API Layer
        API --> Guards[Request Guards: AuthenticatedUser, RequireRole]
        API --> Fairings[Fairings: CORS, RequestID, SecurityHeaders]
        API --> Catchers[Error Catchers]
    end

    subgraph Application Layer
        Guards --> Services[Application Services]
        Services --> Commands[Command Handlers]
        Services --> Queries[Query Handlers]
        Services --> DTO[DTO Mappers]
    end

    subgraph Domain Layer
        Services --> DomainModels[Domain Entities & RBAC Enums]
    end

    subgraph Infrastructure Layer
        Services --> Repositories[SQLx Repository Implementations]
        Services --> AuthInfra[Argon2id & JWT Token Service]
        Services --> StorageInfra[LocalStorageProvider / Trait]
        Repositories --> Postgres[(PostgreSQL 17)]
    end
```

## 2. Authentication & Refresh Flow

```mermaid
sequenceDiagram
    autonumber
    actor Admin as Admin Client
    participant API as Rocket Auth Handler
    participant AuthService as Auth Service
    participant DB as PostgreSQL
    
    Admin->>API: POST /api/v1/auth/login { email, password }
    API->>AuthService: Validate Credentials
    AuthService->>DB: Query User by Email
    DB-->>AuthService: User entity with Argon2id hash
    AuthService->>AuthService: Verify Argon2id Password
    AuthService->>AuthService: Generate Access Token (JWT 15m) & Refresh Token (30d)
    AuthService->>AuthService: Hash Refresh Token (SHA-256)
    AuthService->>DB: INSERT into user_refresh_tokens
    AuthService-->>API: User DTO + Tokens
    API-->>Admin: 200 OK { data: { access_token, refresh_token, user } }

    Note over Admin, API: Access Token Expires
    Admin->>API: POST /api/v1/auth/refresh { refresh_token }
    API->>AuthService: Rotate Refresh Token
    AuthService->>DB: Find non-revoked active refresh token by hash
    DB-->>AuthService: Token record
    AuthService->>DB: Revoke old refresh token (set revoked_at = NOW)
    AuthService->>AuthService: Issue new Access & Refresh tokens
    AuthService->>DB: INSERT new refresh token hash
    AuthService-->>API: New Tokens
    API-->>Admin: 200 OK { data: { access_token, refresh_token } }
```

## 3. Media Upload & Storage Trait Flow

```mermaid
sequenceDiagram
    autonumber
    actor Editor as Editor
    participant MediaAPI as Rocket Media Handler
    participant MediaService as Media Service
    participant Storage as LocalStorageProvider
    participant DB as PostgreSQL
    
    Editor->>MediaAPI: POST /api/v1/admin/media/upload (multipart/form-data)
    MediaAPI->>MediaService: Process Upload Stream
    MediaService->>MediaService: Validate declared & magic byte MIME type
    MediaService->>Storage: upload(temp_path, original_name)
    Storage-->>MediaService: Storage Key / Path (UUID filename)
    MediaService->>DB: INSERT INTO media_assets
    DB-->>MediaService: Saved Media Asset record
    MediaService-->>MediaAPI: Media Asset DTO
    MediaAPI-->>Editor: 201 Created { data: MediaAsset }
```

## 4. Public Page Query & ETag Flow

```mermaid
sequenceDiagram
    autonumber
    actor Visitor as Public Visitor / CDN
    participant PubAPI as Rocket Public Handler
    participant PubService as Public Service
    participant DB as PostgreSQL
    
    Visitor->>PubAPI: GET /api/v1/public/page (If-None-Match: "etag_hash")
    PubAPI->>PubService: Get Aggregated Home Page Data
    PubService->>DB: Fetch Published Home Page, Sections, Content Items, Media Assets, Settings, Social Links
    DB-->>PubService: Database Entities
    PubService->>PubService: Map to Public DTOs & Compute ETag
    alt ETag matches If-None-Match
        PubAPI-->>Visitor: 304 Not Modified
    else Content Modified or No If-None-Match
        PubAPI-->>Visitor: 200 OK { data: PublicPageResponse } + ETag Header
    end
```
