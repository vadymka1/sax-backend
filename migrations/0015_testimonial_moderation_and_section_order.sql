-- Migration 0015: Add testimonial moderation status, submission source, and align canonical section order
-- Canonical section order: About Us (10), Gallery (20), Testimonials (30), Contact Us (40)

-- 1. Add moderation_status and submission_source columns with safe check constraints
ALTER TABLE testimonials
ADD COLUMN IF NOT EXISTS moderation_status VARCHAR(20) DEFAULT 'pending' CHECK (moderation_status IN ('pending', 'approved', 'rejected')),
ADD COLUMN IF NOT EXISTS submission_source VARCHAR(20) DEFAULT 'admin' CHECK (submission_source IN ('admin', 'public'));

-- 2. Backfill existing testimonials as approved admin testimonials
UPDATE testimonials
SET moderation_status = 'approved',
    submission_source = 'admin'
WHERE moderation_status IS NULL OR submission_source IS NULL;

-- 3. Enforce NOT NULL on both columns after backfill
ALTER TABLE testimonials
ALTER COLUMN moderation_status SET NOT NULL,
ALTER COLUMN submission_source SET NOT NULL;

-- 4. Update public query index to filter approved testimonials only
DROP INDEX IF EXISTS idx_testimonials_public;
CREATE INDEX idx_testimonials_public ON testimonials (sort_order ASC, id ASC)
WHERE deleted_at IS NULL AND is_visible = TRUE AND moderation_status = 'approved';

CREATE INDEX IF NOT EXISTS idx_testimonials_moderation_status ON testimonials (moderation_status)
WHERE deleted_at IS NULL;

-- 5. Align canonical SPA sections order on home page
DO $$
DECLARE
    v_page_id UUID;
    v_about_id UUID := '11111111-1111-1111-1111-111111111111';
    v_gall_id  UUID := '44444444-4444-4444-4444-444444444444';
    v_test_id  UUID := '66666666-6666-6666-6666-666666666666';
    v_cont_id  UUID := '55555555-5555-5555-5555-555555555555';
BEGIN
    SELECT id INTO v_page_id FROM pages WHERE slug = 'home' LIMIT 1;
    IF v_page_id IS NOT NULL THEN
        -- Update canonical section sort orders without modifying visibility customizations or IDs
        UPDATE spa_sections SET sort_order = 10, updated_at = NOW() WHERE page_id = v_page_id AND section_key = 'about-us';
        UPDATE spa_sections SET sort_order = 20, updated_at = NOW() WHERE page_id = v_page_id AND section_key = 'gallery';
        UPDATE spa_sections SET sort_order = 30, updated_at = NOW() WHERE page_id = v_page_id AND section_key = 'testimonials';
        UPDATE spa_sections SET sort_order = 40, updated_at = NOW() WHERE page_id = v_page_id AND section_key = 'contact-us';

        -- If any canonical section is missing entirely, ensure it exists
        INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible)
        VALUES (v_about_id, v_page_id, 'about-us', 'About Us', 'About Us', 10, TRUE)
        ON CONFLICT (page_id, section_key) DO NOTHING;

        INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible)
        VALUES (v_gall_id, v_page_id, 'gallery', 'Gallery', 'Gallery', 20, TRUE)
        ON CONFLICT (page_id, section_key) DO NOTHING;

        INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible)
        VALUES (v_test_id, v_page_id, 'testimonials', 'Testimonials', 'Testimonials', 30, TRUE)
        ON CONFLICT (page_id, section_key) DO NOTHING;

        INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible)
        VALUES (v_cont_id, v_page_id, 'contact-us', 'Contact Us', 'Contact Us', 40, TRUE)
        ON CONFLICT (page_id, section_key) DO NOTHING;
    END IF;
END $$;
