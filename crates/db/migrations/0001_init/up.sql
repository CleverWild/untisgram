-- Create tables for Diesel schema
-- Note: dev-only; adjust constraints as needed for production

CREATE EXTENSION IF NOT EXISTS "uuid-ossp"; -- harmless if not available; needed only if you add defaults

CREATE TABLE IF NOT EXISTS bot_states (
    id uuid PRIMARY KEY,
    status_message_id integer NULL,
    target_class_name text NULL,
    untis_school text NOT NULL,
    task_name text NOT NULL,
    untis_login text NOT NULL,
    untis_password text NOT NULL,
    updated_at timestamptz NOT NULL,
    notification_chat_id bigint NOT NULL,
    notification_thread_id integer NULL,
    status_chat_id bigint NOT NULL,
    status_thread_id integer NULL,
    engaged_at timestamptz NOT NULL
);

CREATE TABLE IF NOT EXISTS lessons (
    id uuid PRIMARY KEY,
    bot_state uuid NULL REFERENCES bot_states(id) ON DELETE SET NULL,
    lesson_id bigint NOT NULL,
    date date NOT NULL,
    end_time time NOT NULL,
    lesson_type text NULL,
    start_time time NOT NULL,
    subst_text text NULL,
    lesson_code text NOT NULL,
    classes text[] NOT NULL,
    rooms text[] NOT NULL,
    subjects text[] NOT NULL,
    teachers text[] NOT NULL
);
