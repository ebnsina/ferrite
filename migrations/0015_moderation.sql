-- Content moderation result for an asset (from transcript classification).
CREATE TABLE moderation (
    asset_id    UUID PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    tenant_id   UUID NOT NULL,
    flagged     BOOLEAN NOT NULL,
    categories  JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
