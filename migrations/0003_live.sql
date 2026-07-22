-- Live streams: RTMP ingest keyed by a secret stream key, HLS playback.
CREATE TABLE live_streams (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    stream_key  TEXT NOT NULL UNIQUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_live_streams_tenant ON live_streams(tenant_id);
