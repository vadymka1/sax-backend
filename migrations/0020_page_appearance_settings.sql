-- Migration 0020: Page appearance settings (Backend Page Appearance V1)
-- Introduces singleton appearance settings for pages (starting with 'home')
-- Supporting background image (referencing media_assets), overlay opacity, position, and size.

CREATE TABLE IF NOT EXISTS page_appearance_settings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    page_id UUID NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    background_media_id UUID NULL REFERENCES media_assets(id) ON DELETE SET NULL,
    overlay_opacity DOUBLE PRECISION NOT NULL DEFAULT 0.35 CHECK (overlay_opacity >= 0.0 AND overlay_opacity <= 1.0),
    background_position VARCHAR(20) NOT NULL DEFAULT 'center' CHECK (background_position IN ('center', 'top', 'bottom')),
    background_size VARCHAR(20) NOT NULL DEFAULT 'cover' CHECK (background_size IN ('cover', 'contain')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_page_appearance_settings_page UNIQUE (page_id)
);

CREATE INDEX IF NOT EXISTS idx_page_appearance_settings_page_id ON page_appearance_settings(page_id);
CREATE INDEX IF NOT EXISTS idx_page_appearance_settings_bg_media_id ON page_appearance_settings(background_media_id);

-- Seed initial default appearance settings for existing 'home' page
INSERT INTO page_appearance_settings (page_id, overlay_opacity, background_position, background_size)
SELECT id, 0.35, 'center', 'cover' FROM pages WHERE slug = 'home'
ON CONFLICT (page_id) DO NOTHING;
