-- Early-access waitlist (public signups; doubles as market research).
CREATE TABLE waitlist (
    id          BIGSERIAL PRIMARY KEY,
    name        TEXT NOT NULL,
    email       TEXT NOT NULL,
    whatsapp    TEXT,
    country     TEXT,
    use_case    TEXT,
    volume      TEXT,
    plan        TEXT,
    payment     TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_waitlist_created ON waitlist (created_at DESC);
