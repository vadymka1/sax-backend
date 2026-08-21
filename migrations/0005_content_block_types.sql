-- Forward Migration: Allow content block types text_image, text_youtube, text_video in sections table and drop uq_sections_page_key constraint

ALTER TABLE sections DROP CONSTRAINT IF EXISTS uq_sections_page_key;
ALTER TABLE sections DROP CONSTRAINT IF EXISTS sections_section_type_check;

ALTER TABLE sections ADD CONSTRAINT sections_section_type_check
CHECK (section_type IN ('hero', 'text', 'text_image', 'text_youtube', 'text_video', 'image_text', 'services', 'gallery', 'video', 'contact', 'social_links', 'cta', 'footer', 'custom'));
