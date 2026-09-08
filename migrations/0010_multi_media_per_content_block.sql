-- Migration 0010: Support multiple media assets per content block (carousel support)
-- Forward migration: Replace single media per block constraint with (section_id, media_asset_id) uniqueness

-- 1. Drop one-media-per-block restriction
ALTER TABLE section_media DROP CONSTRAINT IF EXISTS uq_section_media_single;

-- 2. Deduplicate any duplicate (section_id, media_asset_id) pairs if present
DELETE FROM section_media sm1
WHERE sm1.id IN (
    SELECT sm2.id
    FROM section_media sm2
    JOIN section_media sm3 
      ON sm2.section_id = sm3.section_id 
     AND sm2.media_asset_id = sm3.media_asset_id 
     AND sm2.id > sm3.id
);

-- 3. Add constraint preventing duplicate attachment of the same media asset to the same block
ALTER TABLE section_media DROP CONSTRAINT IF EXISTS uq_section_media_block_asset;
ALTER TABLE section_media ADD CONSTRAINT uq_section_media_block_asset UNIQUE (section_id, media_asset_id);

-- 4. Index for deterministic sort ordering on attached media
CREATE INDEX IF NOT EXISTS idx_section_media_sort_order ON section_media (section_id, sort_order ASC, created_at ASC);
