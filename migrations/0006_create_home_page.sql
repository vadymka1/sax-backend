-- Migration: Create system default home page idempotently

INSERT INTO pages (id, slug, title, status)
VALUES (uuid_generate_v4(), 'home', 'Home', 'published')
ON CONFLICT (slug) DO NOTHING;
