-- Idempotent-ish restore: add column if not exists, then (re)add FK constraint
ALTER TABLE logs ADD COLUMN IF NOT EXISTS bot_state uuid;

-- Restore foreign key constraint
ALTER TABLE logs
	ADD CONSTRAINT logs_bot_state_fkey
	FOREIGN KEY (bot_state)
	REFERENCES bot_states(id)
	ON DELETE CASCADE;
-- This file should undo anything in `up.sql`
