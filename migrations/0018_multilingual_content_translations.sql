-- Migration 0018: Add translation tables for SPA sections and content blocks (V1: English and German)
-- Preserves existing tables and columns, backfills existing data into English ('en') translation rows.

-- 1. Create spa_section_translations table
CREATE TABLE IF NOT EXISTS spa_section_translations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    spa_section_id UUID NOT NULL REFERENCES spa_sections(id) ON DELETE CASCADE,
    locale VARCHAR(10) NOT NULL CHECK (locale IN ('en', 'de')),
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_spa_section_translations UNIQUE (spa_section_id, locale)
);

CREATE INDEX IF NOT EXISTS idx_spa_section_translations_lookup
    ON spa_section_translations(spa_section_id, locale);

-- 2. Create content_block_translations table (logical ContentBlock -> physical sections table)
CREATE TABLE IF NOT EXISTS content_block_translations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    content_block_id UUID NOT NULL REFERENCES sections(id) ON DELETE CASCADE,
    locale VARCHAR(10) NOT NULL CHECK (locale IN ('en', 'de')),
    title VARCHAR(200) NULL,
    text TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_content_block_translations UNIQUE (content_block_id, locale)
);

CREATE INDEX IF NOT EXISTS idx_content_block_translations_lookup
    ON content_block_translations(content_block_id, locale);

-- 3. Backfill existing production SpaSection content as English ('en')
INSERT INTO spa_section_translations (spa_section_id, locale, name, created_at, updated_at)
SELECT
    id,
    'en',
    title,
    created_at,
    updated_at
FROM spa_sections
ON CONFLICT (spa_section_id, locale) DO UPDATE
SET name = EXCLUDED.name,
    updated_at = EXCLUDED.updated_at;

-- 4. Backfill existing production ContentBlock (sections) content as English ('en')
INSERT INTO content_block_translations (content_block_id, locale, title, text, created_at, updated_at)
SELECT
    id,
    'en',
    title,
    COALESCE(content->>'text', ''),
    created_at,
    updated_at
FROM sections
ON CONFLICT (content_block_id, locale) DO UPDATE
SET title = EXCLUDED.title,
    text = EXCLUDED.text,
    updated_at = EXCLUDED.updated_at;
