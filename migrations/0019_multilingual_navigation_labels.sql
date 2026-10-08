-- Migration 0019: Multilingual navigation labels (V1.2: Localized navigation labels)
-- Extends spa_section_translations with nullable navigation_label and backfills English ('en') rows from legacy spa_sections.

-- 1. Add navigation_label column to spa_section_translations
ALTER TABLE spa_section_translations
ADD COLUMN IF NOT EXISTS navigation_label VARCHAR(100) NULL;

-- 2. Safely backfill English ('en') translation rows from existing spa_sections.navigation_label
-- Idempotent: Only updates rows where navigation_label IS NULL, preserving any existing translated value
UPDATE spa_section_translations t
SET navigation_label = s.navigation_label
FROM spa_sections s
WHERE t.spa_section_id = s.id
  AND t.locale = 'en'
  AND t.navigation_label IS NULL;
