-- Per-job AES-128 key for HLS encryption. NULL = unencrypted output.
-- Served only via the authorized playback key endpoint, never in storage.
ALTER TABLE jobs ADD COLUMN encryption_key BYTEA;
