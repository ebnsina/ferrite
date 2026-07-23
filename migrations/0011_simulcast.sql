-- Simulcast/restream destinations for a live stream (RTMP fan-out targets).
CREATE TABLE simulcast_targets (
    id             UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    live_stream_id UUID NOT NULL REFERENCES live_streams(id) ON DELETE CASCADE,
    tenant_id      UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name           TEXT NOT NULL,
    url            TEXT NOT NULL,        -- e.g. rtmp://a.rtmp.youtube.com/live2
    stream_key     TEXT NOT NULL,        -- destination stream key
    enabled        BOOLEAN NOT NULL DEFAULT true,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_simulcast_stream ON simulcast_targets(live_stream_id);
