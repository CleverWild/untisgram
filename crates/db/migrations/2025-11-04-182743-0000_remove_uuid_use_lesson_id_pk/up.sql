-- Remove the primary key constraint on id
ALTER TABLE lessons DROP CONSTRAINT lessons_pkey;

-- Drop the id column
ALTER TABLE lessons DROP COLUMN id;

-- Add primary key constraint on lesson_id
ALTER TABLE lessons ADD PRIMARY KEY (lesson_id);
