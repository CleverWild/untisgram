-- Remove the primary key constraint on lesson_id
ALTER TABLE lessons DROP CONSTRAINT lessons_pkey;

-- Add back the id column
ALTER TABLE lessons ADD COLUMN id UUID;

-- Update id column with new UUIDs (required for making it primary key)
UPDATE lessons SET id = gen_random_uuid();

-- Make id NOT NULL
ALTER TABLE lessons ALTER COLUMN id SET NOT NULL;

-- Add primary key constraint back on id
ALTER TABLE lessons ADD PRIMARY KEY (id);

