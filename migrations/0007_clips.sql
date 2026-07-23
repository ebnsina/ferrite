-- Jobs can now be transcodes or clips. A clip job trims a source asset into a
-- new asset (dest_asset_id); output_prefix stays empty for clips.
ALTER TABLE jobs ADD COLUMN kind TEXT NOT NULL DEFAULT 'transcode';
ALTER TABLE jobs ADD COLUMN dest_asset_id UUID REFERENCES assets(id) ON DELETE SET NULL;
