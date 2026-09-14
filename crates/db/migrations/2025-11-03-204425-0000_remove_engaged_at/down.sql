-- Add back engaged_at column
ALTER TABLE bot_states ADD COLUMN engaged_at timestamptz NOT NULL DEFAULT NOW();
