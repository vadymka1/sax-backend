-- Migration 0009: Harden SPA sections schema, constraints, same-page ownership, and NOT NULL invariants

-- 1. Add CHECK constraints to spa_sections for section_key format and non-empty title/navigation_label
ALTER TABLE spa_sections ADD CONSTRAINT chk_spa_sections_key_format 
    CHECK (section_key ~ '^[a-z0-9]+(-[a-z0-9]+)*$');

ALTER TABLE spa_sections ADD CONSTRAINT chk_spa_sections_title_non_empty 
    CHECK (length(btrim(title)) > 0);

ALTER TABLE spa_sections ADD CONSTRAINT chk_spa_sections_nav_label_non_empty 
    CHECK (length(btrim(navigation_label)) > 0);

-- 2. Ensure composite unique constraint (id, page_id) on spa_sections for same-page FK integrity
ALTER TABLE spa_sections ADD CONSTRAINT uq_spa_sections_id_page UNIQUE (id, page_id);

-- 3. Fail fast if non-home legacy pages contain active or soft-deleted content blocks
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM sections s
        JOIN pages p ON p.id = s.page_id
        WHERE p.slug <> 'home'
    ) THEN
        RAISE EXCEPTION 'Cannot migrate legacy content blocks from non-home pages into single-page SPA model';
    END IF;
END $$;

-- 4. Backfill any remaining NULL spa_section_id values specifically for home page content blocks
DO $$
DECLARE
    v_home_about_id UUID;
BEGIN
    SELECT s.id INTO v_home_about_id
    FROM spa_sections s
    JOIN pages p ON s.page_id = p.id
    WHERE p.slug = 'home' AND s.section_key = 'about-us'
    LIMIT 1;

    IF v_home_about_id IS NOT NULL THEN
        UPDATE sections s
        SET spa_section_id = v_home_about_id
        FROM pages p
        WHERE s.page_id = p.id
          AND p.slug = 'home'
          AND s.spa_section_id IS NULL;
    END IF;
END $$;

-- 5. Verify no content blocks (active or soft-deleted) have NULL spa_section_id, else fail migration
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM sections WHERE spa_section_id IS NULL
    ) THEN
        RAISE EXCEPTION 'Cannot complete SPA sections hardening migration: content blocks without spa_section_id remain';
    END IF;
END $$;

-- 6. Unconditionally enforce NOT NULL constraint on sections.spa_section_id
ALTER TABLE sections ALTER COLUMN spa_section_id SET NOT NULL;

-- 7. Add same-page composite foreign key on sections (spa_section_id, page_id) -> spa_sections (id, page_id)
ALTER TABLE sections ADD CONSTRAINT fk_sections_spa_section_same_page
    FOREIGN KEY (spa_section_id, page_id)
    REFERENCES spa_sections(id, page_id)
    ON DELETE CASCADE;
