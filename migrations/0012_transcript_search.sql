-- Searchable transcript segments (from auto-captions). Full-text now; a vector
-- column can be added later for semantic search without touching this schema.
CREATE TABLE transcript_segments (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   UUID NOT NULL,
    asset_id    UUID NOT NULL,
    job_id      UUID NOT NULL,
    start_secs  REAL NOT NULL,
    end_secs    REAL NOT NULL,
    text        TEXT NOT NULL,
    tsv         tsvector GENERATED ALWAYS AS (to_tsvector('english', text)) STORED,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_transcript_tsv ON transcript_segments USING GIN (tsv);
CREATE INDEX idx_transcript_tenant ON transcript_segments (tenant_id);
CREATE INDEX idx_transcript_job ON transcript_segments (job_id);
