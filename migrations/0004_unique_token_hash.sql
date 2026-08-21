-- Forward Migration: Add unique constraint on user_refresh_tokens.token_hash

ALTER TABLE user_refresh_tokens ADD CONSTRAINT uq_user_refresh_tokens_hash UNIQUE (token_hash);
