-- Revert NOT NULL and default
ALTER TABLE lessons ALTER COLUMN lesson_type DROP NOT NULL;
ALTER TABLE lessons ALTER COLUMN lesson_type DROP DEFAULT;