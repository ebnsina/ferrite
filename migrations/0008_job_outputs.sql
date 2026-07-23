-- Extra deliverables a transcode may produce, so playback URLs can be offered.
ALTER TABLE jobs ADD COLUMN has_mp4 BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE jobs ADD COLUMN has_audio BOOLEAN NOT NULL DEFAULT false;
