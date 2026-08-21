-- Migration 0008: Add spa_sections table and link sections (content_blocks) to spa_sections

-- 1. Create spa_sections table
CREATE TABLE IF NOT EXISTS spa_sections (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    page_id UUID NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    section_key VARCHAR(100) NOT NULL,
    title VARCHAR(255) NOT NULL,
    navigation_label VARCHAR(100) NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0 CHECK (sort_order >= 0),
    is_visible BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ NULL,
    CONSTRAINT uq_spa_sections_page_key UNIQUE (page_id, section_key)
);

-- 2. Indexes for spa_sections
CREATE INDEX IF NOT EXISTS idx_spa_sections_page_id ON spa_sections(page_id);
CREATE INDEX IF NOT EXISTS idx_spa_sections_sort_order ON spa_sections(sort_order);
CREATE INDEX IF NOT EXISTS idx_spa_sections_is_visible ON spa_sections(is_visible);
CREATE INDEX IF NOT EXISTS idx_spa_sections_deleted_at ON spa_sections(deleted_at);
CREATE INDEX IF NOT EXISTS idx_spa_sections_page_sort ON spa_sections(page_id, sort_order);

-- 3. Seed initial 5 default SPA navigation sections for the 'home' page
DO $$
DECLARE
    v_page_id UUID;
    v_about_id UUID := '11111111-1111-1111-1111-111111111111';
    v_works_id UUID := '22222222-2222-2222-2222-222222222222';
    v_fest_id UUID  := '33333333-3333-3333-3333-333333333333';
    v_gall_id UUID  := '44444444-4444-4444-4444-444444444444';
    v_cont_id UUID  := '55555555-5555-5555-5555-555555555555';
BEGIN
    SELECT id INTO v_page_id FROM pages WHERE slug = 'home' LIMIT 1;
    IF v_page_id IS NOT NULL THEN
        INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order)
        VALUES
            (v_about_id, v_page_id, 'about-us', 'About Us', 'About Us', 10),
            (v_works_id, v_page_id, 'our-works', 'Our Works', 'Our Works', 20),
            (v_fest_id,  v_page_id, 'festivals', 'Festivals', 'Festivals', 30),
            (v_gall_id,  v_page_id, 'gallery',   'Gallery',   'Gallery',   40),
            (v_cont_id,  v_page_id, 'contact-us', 'Contact Us', 'Contact Us', 50)
        ON CONFLICT (page_id, section_key) DO NOTHING;
    END IF;
END $$;

-- 4. Add nullable spa_section_id column to physical sections table (ContentBlock)
ALTER TABLE sections ADD COLUMN IF NOT EXISTS spa_section_id UUID REFERENCES spa_sections(id);

-- 5. Safely backfill existing content blocks to 'about-us' spa_section ONLY for 'home' page content blocks
DO $$
DECLARE
    v_default_spa_id UUID;
BEGIN
    SELECT s.id INTO v_default_spa_id
    FROM spa_sections s
    JOIN pages p ON s.page_id = p.id
    WHERE p.slug = 'home' AND s.section_key = 'about-us'
    LIMIT 1;

    IF v_default_spa_id IS NOT NULL THEN
        UPDATE sections s
        SET spa_section_id = v_default_spa_id
        FROM pages p
        WHERE s.page_id = p.id
          AND p.slug = 'home'
          AND s.spa_section_id IS NULL;
    END IF;
END $$;

-- 6. Add index for sections.spa_section_id
CREATE INDEX IF NOT EXISTS idx_sections_spa_section_id ON sections(spa_section_id);
