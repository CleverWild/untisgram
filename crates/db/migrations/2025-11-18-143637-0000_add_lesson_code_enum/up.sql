-- Create PostgreSQL enum type for lesson_code
CREATE TYPE lesson_code AS ENUM ('regular', 'irregular', 'cancelled');

-- Alter lessons.lesson_code from text to enum
ALTER TABLE lessons
	ALTER COLUMN lesson_code TYPE lesson_code USING lesson_code::lesson_code;
-- Your SQL goes here
