-- Forward Migration: Add CITEXT extension for case-insensitive unique email handling

CREATE EXTENSION IF NOT EXISTS citext;

ALTER TABLE users ALTER COLUMN email TYPE CITEXT;
