-- Migration 0011: Align default SPA sections to the 4 canonical sections (About Us, Gallery, Contact Us, Testimonials)
-- Ensures all 4 canonical sections exist with exact sort orders and visibility
-- Preserves user-created sections and hides old default sections without deleting user content

DO $$
DECLARE
    v_page_id UUID;
    v_about_id UUID := '11111111-1111-1111-1111-111111111111';
    v_gall_id  UUID := '44444444-4444-4444-4444-444444444444';
    v_cont_id  UUID := '55555555-5555-5555-5555-555555555555';
    v_test_id  UUID := '66666666-6666-6666-6666-666666666666';
BEGIN
    SELECT id INTO v_page_id FROM pages WHERE slug = 'home' LIMIT 1;
    IF v_page_id IS NOT NULL THEN
        -- 1. Ensure 'about-us' (10)
        INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible)
        VALUES (v_about_id, v_page_id, 'about-us', 'About Us', 'About Us', 10, TRUE)
        ON CONFLICT (page_id, section_key) DO UPDATE
        SET title = EXCLUDED.title,
            navigation_label = EXCLUDED.navigation_label,
            sort_order = 10,
            is_visible = TRUE,
            deleted_at = NULL;

        -- 2. Ensure 'gallery' (20)
        INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible)
        VALUES (v_gall_id, v_page_id, 'gallery', 'Gallery', 'Gallery', 20, TRUE)
        ON CONFLICT (page_id, section_key) DO UPDATE
        SET title = EXCLUDED.title,
            navigation_label = EXCLUDED.navigation_label,
            sort_order = 20,
            is_visible = TRUE,
            deleted_at = NULL;

        -- 3. Ensure 'contact-us' (30)
        INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible)
        VALUES (v_cont_id, v_page_id, 'contact-us', 'Contact Us', 'Contact Us', 30, TRUE)
        ON CONFLICT (page_id, section_key) DO UPDATE
        SET title = EXCLUDED.title,
            navigation_label = EXCLUDED.navigation_label,
            sort_order = 30,
            is_visible = TRUE,
            deleted_at = NULL;

        -- 4. Ensure 'testimonials' (40)
        INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible)
        VALUES (v_test_id, v_page_id, 'testimonials', 'Testimonials', 'Testimonials', 40, TRUE)
        ON CONFLICT (page_id, section_key) DO UPDATE
        SET title = EXCLUDED.title,
            navigation_label = EXCLUDED.navigation_label,
            sort_order = 40,
            is_visible = TRUE,
            deleted_at = NULL;

        -- 5. Safely hide old default sections (our-works, festivals) on home page
        -- We do NOT delete them so any content blocks attached are preserved safely.
        UPDATE spa_sections
        SET is_visible = FALSE
        WHERE page_id = v_page_id AND section_key IN ('our-works', 'festivals');
    END IF;
END $$;
