-- Remove timezone field from bot_states table
ALTER TABLE bot_states
DROP COLUMN timezone;
