-- Migration 0017: Add read/unread state to contact_messages for admin inbox

ALTER TABLE contact_messages
ADD COLUMN is_read BOOLEAN NOT NULL DEFAULT FALSE,
ADD COLUMN read_at TIMESTAMPTZ NULL;

CREATE INDEX IF NOT EXISTS idx_contact_messages_read_status
ON contact_messages (is_read, created_at DESC);
