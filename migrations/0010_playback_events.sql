-- Lightweight playback analytics: view sessions, watch-time heartbeats, completions.
CREATE TABLE playback_events (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   UUID NOT NULL,
    job_id      UUID NOT NULL,
    session     TEXT NOT NULL,
    kind        TEXT NOT NULL,               -- view | heartbeat | ended
    position    REAL NOT NULL DEFAULT 0,     -- playhead seconds
    watched     REAL NOT NULL DEFAULT 0,     -- seconds watched since last beacon
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_playback_events_job ON playback_events(job_id);
