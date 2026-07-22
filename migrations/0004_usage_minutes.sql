-- Store metered minutes as a float (transcode minutes accrue fractionally).
ALTER TABLE usage ALTER COLUMN minutes TYPE double precision;
