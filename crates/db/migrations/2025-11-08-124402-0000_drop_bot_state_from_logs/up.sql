-- Idempotent safe drop: remove FK constraint if exists then drop column if exists
ALTER TABLE logs DROP CONSTRAINT IF EXISTS logs_bot_state_fkey;
ALTER TABLE logs DROP COLUMN IF EXISTS bot_state;
