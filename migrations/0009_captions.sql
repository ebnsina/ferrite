-- A transcode may also produce a WebVTT captions track.
ALTER TABLE jobs ADD COLUMN has_captions BOOLEAN NOT NULL DEFAULT false;
