-- Create logs table for application events
CREATE TABLE IF NOT EXISTS logs (
    id uuid PRIMARY KEY,
    ts timestamptz NOT NULL DEFAULT now(),
    level text NOT NULL,
    target text NULL,
    message text NOT NULL,
    fields jsonb NULL,
    file text NULL,
    line int4 NULL
);

-- Indexes to speed up common queries
CREATE INDEX IF NOT EXISTS idx_logs_ts ON logs (ts DESC);
CREATE INDEX IF NOT EXISTS idx_logs_level_ts ON logs (level, ts DESC);
