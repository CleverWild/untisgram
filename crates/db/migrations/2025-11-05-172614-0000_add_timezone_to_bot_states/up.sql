-- Add timezone field to bot_states table
ALTER TABLE bot_states
ADD COLUMN timezone TEXT NOT NULL DEFAULT 'Europe/Berlin';
