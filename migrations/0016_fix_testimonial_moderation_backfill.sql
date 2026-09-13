-- Migration 0016: Repair testimonial moderation backfill for historical admin rows
-- In migration 0015, moderation_status was added with DEFAULT 'pending', which caused
-- pre-existing testimonials to receive 'pending' before the backfill condition ran.
-- This migration safely repairs those historical admin rows by setting them to 'approved'.
-- Only rows with moderation_status = 'pending' AND submission_source = 'admin' are repaired.
-- Real public pending submissions (submission_source = 'public') remain pending.
-- Already approved or rejected rows remain untouched.
-- Visibility, sort_order, deleted_at, and all other fields are preserved.

UPDATE testimonials
SET moderation_status = 'approved'
WHERE moderation_status = 'pending'
  AND submission_source = 'admin';
