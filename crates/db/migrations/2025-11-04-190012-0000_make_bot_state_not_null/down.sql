-- Allow NULLs again
ALTER TABLE lessons ALTER COLUMN bot_state DROP NOT NULL;
