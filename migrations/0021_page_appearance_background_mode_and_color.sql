-- Migration 0021: Page appearance background mode and color (Backend Page Appearance V1.1)
-- Extends page_appearance_settings with explicit background_mode ('none', 'color', 'image')
-- and solid background_color (persisted uppercase hex #RRGGBB).

ALTER TABLE page_appearance_settings
    ADD COLUMN IF NOT EXISTS background_mode VARCHAR(20) NOT NULL DEFAULT 'none'
        CHECK (background_mode IN ('none', 'color', 'image')),
    ADD COLUMN IF NOT EXISTS background_color VARCHAR(7) NOT NULL DEFAULT '#FFFFFF'
        CHECK (background_color ~* '^#[0-9a-f]{6}$');

-- Backfill background_mode for existing rows:
-- If background_media_id is present, set to 'image'. Otherwise, set to 'none'.
UPDATE page_appearance_settings
SET background_mode = CASE
    WHEN background_media_id IS NOT NULL THEN 'image'
    ELSE 'none'
END;
