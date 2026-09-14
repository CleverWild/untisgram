-- Remove orphan lessons (no owning bot_state)
DELETE FROM lessons WHERE bot_state IS NULL;

-- Make bot_state required
ALTER TABLE lessons ALTER COLUMN bot_state SET NOT NULL;
