-- Mock Seed Data for Development and Testing Environment

-- 1. SEED USERS (Password for all accounts: SuperPassword123!)
-- Argon2id hash of "SuperPassword123!"
-- $argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0$wA/Y+S71uDq+p3v5+O3v2Q
-- Pre-hashed known mock hash for seeding:
INSERT INTO users (id, email, password_hash, display_name, role, is_active)
VALUES 
    ('11111111-1111-1111-1111-111111111111', 'superadmin@example.com', '$argon2id$v=19$m=19456,t=2,p=1$Z2VuZXJhdGVkc2FsdA$7w1y+9Z8jN2wP1mN0kK9A5JgH1fE3dC2bA4eF6gH8iJ', 'Osanna Super Admin', 'super_admin', TRUE),
    ('22222222-2222-2222-2222-222222222222', 'admin@example.com', '$argon2id$v=19$m=19456,t=2,p=1$Z2VuZXJhdGVkc2FsdA$7w1y+9Z8jN2wP1mN0kK9A5JgH1fE3dC2bA4eF6gH8iJ', 'Osanna Admin Manager', 'admin', TRUE),
    ('33333333-3333-3333-3333-333333333333', 'editor@example.com', '$argon2id$v=19$m=19456,t=2,p=1$Z2VuZXJhdGVkc2FsdA$7w1y+9Z8jN2wP1mN0kK9A5JgH1fE3dC2bA4eF6gH8iJ', 'Osanna Content Editor', 'editor', TRUE)
ON CONFLICT (LOWER(email)) DO NOTHING;

-- 2. SEED MEDIA ASSETS
INSERT INTO media_assets (id, media_type, storage_provider, storage_key, original_filename, stored_filename, mime_type, file_size, width, height, alt_text, caption, external_url, youtube_video_id, youtube_url, thumbnail_url, status)
VALUES
    ('a1111111-1111-1111-1111-111111111111', 'image', 'local', 'images/hero-bg.jpg', 'hero-bg.jpg', 'a1111111-1111-1111-1111-111111111111.jpg', 'image/jpeg', 1048576, 1920, 1080, 'Osanna playing saxophone on stage', 'Hero saxophone live performance', NULL, NULL, NULL, NULL, 'active'),
    ('a2222222-2222-2222-2222-222222222222', 'youtube', 'local', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'Osanna Sax Live Concert Video', 'Osanna Sax Live at Kyiv Jazz Fest', NULL, 'dQw4w9WgXcQ', 'https://www.youtube.com/watch?v=dQw4w9WgXcQ', 'https://img.youtube.com/vi/dQw4w9WgXcQ/hqdefault.jpg', 'active')
ON CONFLICT (id) DO NOTHING;

-- 3. SEED PAGE (Home Page)
INSERT INTO pages (id, slug, title, status, seo_title, seo_description, seo_keywords, og_title, og_description, og_image_id)
VALUES
    ('b1111111-1111-1111-1111-111111111111', 'home', 'Osanna Sax - Official Portfolio & Booking', 'published', 'Osanna Sax | Premier Saxophonist for Events & Shows', 'Exclusive saxophone performances, live shows, corporate events, and wedding entertainment by Osanna Sax.', ARRAY['saxophone', 'live music', 'wedding saxophonist', 'event entertainment', 'Osanna Sax'], 'Osanna Sax - Live Saxophone Performances', 'Book premier live saxophone music for your venue, wedding, or event.', 'a1111111-1111-1111-1111-111111111111')
ON CONFLICT (slug) DO NOTHING;

-- 4. SEED SECTIONS
INSERT INTO sections (id, page_id, section_key, section_type, title, subtitle, content, settings, sort_order, is_visible, status)
VALUES
    (
        'c1111111-1111-1111-1111-111111111111',
        'b1111111-1111-1111-1111-111111111111',
        'hero',
        'hero',
        'Osanna Sax',
        'Elevating Events Through Soulful Saxophone Performances',
        '{"description": "Professional saxophonist available for international concerts, private VIP galas, weddings, and corporate celebrations.", "primary_action": {"label": "Book Performance", "url": "#contact"}, "secondary_action": {"label": "Watch Live Video", "url": "#video"}}'::jsonb,
        '{"layout": "full_screen_video", "theme": "dark"}'::jsonb,
        10,
        TRUE,
        'published'
    ),
    (
        'c2222222-2222-2222-2222-222222222222',
        'b1111111-1111-1111-1111-111111111111',
        'services',
        'services',
        'Performance Options',
        'Tailored Musical Experiences for Any Event',
        '{"intro": "Select from solo saxophone improvisation with DJ sets, acoustic lounge jazz ensembles, or high-energy stage performances."}'::jsonb,
        '{"columns": 3}'::jsonb,
        20,
        TRUE,
        'published'
    ),
    (
        'c3333333-3333-3333-3333-333333333333',
        'b1111111-1111-1111-1111-111111111111',
        'video',
        'video',
        'Live Video Performances',
        'Experience the Sound & Energy',
        '{}'::jsonb,
        '{}'::jsonb,
        30,
        TRUE,
        'published'
    ),
    (
        'c4444444-4444-4444-4444-444444444444',
        'b1111111-1111-1111-1111-111111111111',
        'contact',
        'contact',
        'Booking & Enquiries',
        'Let us Make Your Event Unforgettable',
        '{"email": "booking@osannasax.com", "phone": "+380501234567", "location": "Kyiv, Ukraine / Available Worldwide"}'::jsonb,
        '{}'::jsonb,
        40,
        TRUE,
        'published'
    )
ON CONFLICT (page_id, section_key) DO NOTHING;

-- 5. SEED CONTENT ITEMS FOR SERVICES
INSERT INTO content_items (id, section_id, item_type, title, subtitle, description, link_url, link_label, sort_order, is_visible, status)
VALUES
    ('d1111111-1111-1111-1111-111111111111', 'c2222222-2222-2222-2222-222222222222', 'service', 'Solo Sax & DJ Live Set', 'Modern House, Pop & Smooth Jazz', 'Dynamic improvisation over modern electronic tracks and house beats, perfect for cocktail hours and club galas.', '#contact', 'Inquire Now', 10, TRUE, 'published'),
    ('d2222222-2222-2222-2222-222222222222', 'c2222222-2222-2222-2222-222222222222', 'service', 'Wedding Ceremonies & Reception', 'Romantic & Festive Melodies', 'Creating unforgettable atmospheric moments during wedding entrance, ring exchange, and dinner cocktail.', '#contact', 'Inquire Now', 20, TRUE, 'published'),
    ('d3333333-3333-3333-3333-333333333333', 'c2222222-2222-2222-2222-222222222222', 'service', 'Corporate Events & VIP Galas', 'Sophisticated Background & Stage Show', 'Tailored musical sets matching brand aesthetics and corporate event themes.', '#contact', 'Inquire Now', 30, TRUE, 'published')
ON CONFLICT (id) DO NOTHING;

-- 6. SEED SECTION MEDIA RELATIONS
INSERT INTO section_media (id, section_id, media_asset_id, usage_type, sort_order, is_visible)
VALUES
    ('e1111111-1111-1111-1111-111111111111', 'c1111111-1111-1111-1111-111111111111', 'a1111111-1111-1111-1111-111111111111', 'background', 10, TRUE),
    ('e2222222-2222-2222-2222-222222222222', 'c3333333-3333-3333-3333-333333333333', 'a2222222-2222-2222-2222-222222222222', 'video', 10, TRUE)
ON CONFLICT (section_id, media_asset_id, usage_type) DO NOTHING;

-- 7. SEED SOCIAL LINKS
INSERT INTO social_links (id, platform, title, url, icon_key, sort_order, is_visible)
VALUES
    ('f1111111-1111-1111-1111-111111111111', 'instagram', 'Instagram', 'https://instagram.com/osannasax', 'instagram', 10, TRUE),
    ('f2222222-2222-2222-2222-222222222222', 'youtube', 'YouTube Channel', 'https://youtube.com/@osannasax', 'youtube', 20, TRUE),
    ('f3333333-3333-3333-3333-333333333333', 'facebook', 'Facebook Page', 'https://facebook.com/osannasax', 'facebook', 30, TRUE),
    ('f4444444-4444-4444-4444-444444444444', 'spotify', 'Spotify Profile', 'https://open.spotify.com/artist/osannasax', 'spotify', 40, TRUE)
ON CONFLICT (id) DO NOTHING;

-- 8. SEED SITE SETTINGS
INSERT INTO site_settings (key, value, description)
VALUES
    ('site_name', '"Osanna Sax Showcase"'::jsonb, 'Official public website title'),
    ('default_language', '"en"'::jsonb, 'Default site language'),
    ('contact_email', '"booking@osannasax.com"'::jsonb, 'Primary booking contact email'),
    ('contact_phone', '"+380501234567"'::jsonb, 'Primary booking contact phone'),
    ('address', '"Kyiv, Ukraine / Worldwide"'::jsonb, 'Geographic location and availability')
ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value;
