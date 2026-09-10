-- Migration 0014: Create testimonials table and ensure canonical testimonials section
-- Dedicated entity for testimonials with optional avatar media reference and canonical ordering

CREATE TABLE IF NOT EXISTS testimonials (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    author_name VARCHAR(120) NOT NULL CHECK (char_length(trim(author_name)) > 0),
    author_role VARCHAR(160) NULL,
    text TEXT NOT NULL CHECK (char_length(trim(text)) > 0),
    avatar_media_id UUID NULL REFERENCES media_assets(id) ON DELETE SET NULL,
    sort_order INTEGER NOT NULL DEFAULT 0 CHECK (sort_order >= 0),
    is_visible BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ NULL
);

-- Indexes for active sorting and public queries
CREATE INDEX IF NOT EXISTS idx_testimonials_active_sort ON testimonials (sort_order ASC, id ASC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_testimonials_public ON testimonials (sort_order ASC, id ASC) WHERE deleted_at IS NULL AND is_visible = TRUE;
CREATE INDEX IF NOT EXISTS idx_testimonials_avatar_media_id ON testimonials (avatar_media_id);

-- Idempotently ensure the canonical 'testimonials' SPA section exists on the home page
DO $$
DECLARE
    v_page_id UUID;
    v_test_id UUID := '66666666-6666-6666-6666-666666666666';
BEGIN
    SELECT id INTO v_page_id FROM pages WHERE slug = 'home' LIMIT 1;
    IF v_page_id IS NOT NULL THEN
        INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible)
        VALUES (v_test_id, v_page_id, 'testimonials', 'Testimonials', 'Testimonials', 40, TRUE)
        ON CONFLICT (page_id, section_key) DO UPDATE
        SET title = EXCLUDED.title,
            navigation_label = EXCLUDED.navigation_label,
            sort_order = 40,
            is_visible = TRUE,
            deleted_at = NULL;
    END IF;
END $$;
