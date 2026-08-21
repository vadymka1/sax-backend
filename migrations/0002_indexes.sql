-- Database Indexes Migration for Query Optimization

CREATE INDEX IF NOT EXISTS idx_users_role ON users (role);
CREATE INDEX IF NOT EXISTS idx_users_is_active ON users (is_active);

CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user_id ON user_refresh_tokens (user_id);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_hash ON user_refresh_tokens (token_hash);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_expires ON user_refresh_tokens (expires_at);

CREATE INDEX IF NOT EXISTS idx_pages_slug ON pages (slug);
CREATE INDEX IF NOT EXISTS idx_pages_status ON pages (status);

CREATE INDEX IF NOT EXISTS idx_sections_page_id ON sections (page_id);
CREATE INDEX IF NOT EXISTS idx_sections_status ON sections (status);
CREATE INDEX IF NOT EXISTS idx_sections_sort_order ON sections (page_id, sort_order);

CREATE INDEX IF NOT EXISTS idx_content_items_section_id ON content_items (section_id);
CREATE INDEX IF NOT EXISTS idx_content_items_sort_order ON content_items (section_id, sort_order);

CREATE INDEX IF NOT EXISTS idx_media_assets_type ON media_assets (media_type);
CREATE INDEX IF NOT EXISTS idx_media_assets_status ON media_assets (status);

CREATE INDEX IF NOT EXISTS idx_section_media_section_id ON section_media (section_id);
CREATE INDEX IF NOT EXISTS idx_section_media_asset_id ON section_media (media_asset_id);

CREATE INDEX IF NOT EXISTS idx_social_links_sort_order ON social_links (sort_order);

CREATE INDEX IF NOT EXISTS idx_audit_logs_actor ON audit_logs (actor_user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_entity ON audit_logs (entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs (created_at DESC);
