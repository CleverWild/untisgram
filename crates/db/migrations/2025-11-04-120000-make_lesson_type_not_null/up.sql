-- Backfill NULLs to a sensible default
UPDATE lessons SET lesson_type = 'Unterricht' WHERE lesson_type IS NULL;

-- Set default for newly inserted rows
ALTER TABLE lessons ALTER COLUMN lesson_type SET DEFAULT 'Unterricht';

-- Make the column NOT NULL
ALTER TABLE lessons ALTER COLUMN lesson_type SET NOT NULL;