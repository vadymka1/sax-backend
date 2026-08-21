-- Forward Migration: Enforce one media asset per content block and support 'content' usage_type

-- Update existing section_media usage_type check constraint to include 'content'
ALTER TABLE section_media DROP CONSTRAINT IF EXISTS section_media_usage_type_check;

ALTER TABLE section_media ADD CONSTRAINT section_media_usage_type_check
CHECK (usage_type IN ('content', 'background', 'cover', 'gallery', 'thumbnail', 'inline', 'video', 'og_image'));

-- Update existing relations to use standard 'content' usage_type
UPDATE section_media SET usage_type = 'content' WHERE usage_type IS NOT NULL;

-- Guard against existing duplicate relations before enforcing UNIQUE(section_id)
DO $$
BEGIN
    IF EXISTS (
        SELECT section_id
        FROM section_media
        GROUP BY section_id
        HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION
            'Cannot enforce one media per block: duplicate section_media rows exist for one or more sections';
    END IF;
END
$$;

-- Enforce one media asset per block via UNIQUE(section_id) constraint
ALTER TABLE section_media DROP CONSTRAINT IF EXISTS uq_section_media;
ALTER TABLE section_media DROP CONSTRAINT IF EXISTS uq_section_media_single;

ALTER TABLE section_media ADD CONSTRAINT uq_section_media_single UNIQUE (section_id);
