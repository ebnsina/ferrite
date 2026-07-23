-- Signed, tamper-evident provenance for produced assets (clips, shorts, live clips).
CREATE TABLE provenance (
    asset_id    UUID PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    tenant_id   UUID NOT NULL,
    manifest    TEXT NOT NULL,     -- exact signed manifest JSON
    signature   TEXT NOT NULL,     -- hex Ed25519 signature
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
