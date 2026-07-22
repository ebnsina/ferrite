-- Ferrite initial schema: tenants, auth, assets, jobs, outputs, metering.
-- Multi-tenant from the start: every tenant-owned row carries tenant_id.

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Tenants ---------------------------------------------------------------------
CREATE TABLE tenants (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name        TEXT NOT NULL,
    plan        TEXT NOT NULL DEFAULT 'free',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Dashboard users -------------------------------------------------------------
CREATE TABLE users (
    id             UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id      UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    email          TEXT NOT NULL UNIQUE,
    password_hash  TEXT NOT NULL,
    role           TEXT NOT NULL DEFAULT 'member',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_users_tenant ON users(tenant_id);

-- API keys (hashed; only the prefix is stored in clear for display) ----------
CREATE TABLE api_keys (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id     UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    key_hash      TEXT NOT NULL UNIQUE,
    prefix        TEXT NOT NULL,
    last_used_at  TIMESTAMPTZ,
    revoked_at    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_api_keys_tenant ON api_keys(tenant_id);

-- Transcode profiles (rendition ladders) -------------------------------------
CREATE TABLE profiles (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id   UUID REFERENCES tenants(id) ON DELETE CASCADE, -- NULL = global preset
    name        TEXT NOT NULL,
    ladder      JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_profiles_tenant ON profiles(tenant_id);

-- Source assets ---------------------------------------------------------------
CREATE TABLE assets (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id     UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    filename      TEXT NOT NULL,
    original_key  TEXT NOT NULL,
    bytes         BIGINT,
    media_info    JSONB,
    status        TEXT NOT NULL DEFAULT 'uploading', -- uploading|ready|error
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_assets_tenant ON assets(tenant_id);

-- Transcode jobs --------------------------------------------------------------
CREATE TABLE jobs (
    id               UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id        UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    asset_id         UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    profile_id       UUID REFERENCES profiles(id) ON DELETE SET NULL,
    state            TEXT NOT NULL DEFAULT 'queued',
    progress         REAL NOT NULL DEFAULT 0,
    priority         INT NOT NULL DEFAULT 0,
    attempts         INT NOT NULL DEFAULT 0,
    error            TEXT,
    idempotency_key  TEXT,
    output_prefix    TEXT NOT NULL,
    queued_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at       TIMESTAMPTZ,
    finished_at      TIMESTAMPTZ,
    UNIQUE (tenant_id, idempotency_key)
);
CREATE INDEX idx_jobs_tenant ON jobs(tenant_id);
CREATE INDEX idx_jobs_state ON jobs(state);

-- Output renditions -----------------------------------------------------------
CREATE TABLE renditions (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    job_id        UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL,          -- hls|dash|thumb
    name          TEXT,                   -- e.g. 720p
    height        INT,
    bitrate_kbps  INT,
    codec         TEXT,
    playlist_key  TEXT,
    prefix        TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_renditions_job ON renditions(job_id);

-- Usage metering (drives mock billing) ---------------------------------------
CREATE TABLE usage (
    id             UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id      UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    period         DATE NOT NULL,
    minutes        NUMERIC NOT NULL DEFAULT 0,
    storage_bytes  BIGINT NOT NULL DEFAULT 0,
    UNIQUE (tenant_id, period)
);

-- Webhooks --------------------------------------------------------------------
CREATE TABLE webhooks (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    url         TEXT NOT NULL,
    secret      TEXT NOT NULL,
    events      TEXT[] NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_webhooks_tenant ON webhooks(tenant_id);
