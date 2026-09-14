-- Revert lessons.lesson_code back to text and drop enum type
ALTER TABLE lessons
	ALTER COLUMN lesson_code TYPE text USING lesson_code::text;
DROP TYPE lesson_code;
-- This file should undo anything in `up.sql`
