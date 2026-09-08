-- Migration 0013: Typography settings for content blocks (font_family and font_size tokens)

ALTER TABLE sections ADD COLUMN IF NOT EXISTS font_family VARCHAR(50) NOT NULL DEFAULT 'sans';
ALTER TABLE sections ADD COLUMN IF NOT EXISTS font_size VARCHAR(50) NOT NULL DEFAULT 'md';

-- Add check constraints to enforce safe typography tokens
ALTER TABLE sections DROP CONSTRAINT IF EXISTS chk_sections_font_family;
ALTER TABLE sections ADD CONSTRAINT chk_sections_font_family
    CHECK (font_family IN ('sans', 'serif', 'display', 'mono'));

ALTER TABLE sections DROP CONSTRAINT IF EXISTS chk_sections_font_size;
ALTER TABLE sections ADD CONSTRAINT chk_sections_font_size
    CHECK (font_size IN ('sm', 'md', 'lg', 'xl', '2xl'));
