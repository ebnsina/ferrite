-- Translated caption tracks available for a job (in addition to the source VTT).
CREATE TABLE caption_tracks (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   UUID NOT NULL,
    job_id      UUID NOT NULL,
    lang        TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (job_id, lang)
);
CREATE INDEX idx_caption_tracks_job ON caption_tracks (job_id);
