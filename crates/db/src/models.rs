// Models for Diesel. Keep these `pub(crate)` so Diesel-related macros/types do not
// become part of the public API of the `db` crate.
//
// The structs below are example mappings to the `table!` definitions in
// `schema.rs`. After generating a real `schema.rs` with `diesel print-schema`
// tweak these to match exact types and use `Insertable` / `AsChangeset` as
// required.

use crate::schema::{bot_states, lessons, logs};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use diesel::{AsChangeset, Identifiable, Insertable, Queryable, Selectable};
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// BotTask represents a bot worker configuration.
#[derive(
    Debug, Clone, Queryable, Identifiable, Selectable, Serialize, Deserialize, restructed::Models,
)]
#[view(
    NewBotTask,
    derive(Insertable, Serialize, Deserialize),
    attributes_with = "deriveless"
)]
#[patch(
    BotTaskChangeset,
    omit(id),
    derive(AsChangeset, Serialize, Deserialize),
    attributes_with = "deriveless"
)]
#[diesel(table_name = bot_states)]
pub struct BotTask {
    pub id: Uuid,
    pub task_name: String,
    pub target_class_name: Option<String>,
    pub timezone: String,
    pub untis_school: String,
    pub untis_login: String,
    pub untis_password: String,
    pub notification_chat_id: i64,
    pub notification_thread_id: Option<i32>,
    pub status_chat_id: i64,
    pub status_thread_id: Option<i32>,
    pub status_message_id: Option<i32>,
    pub updated_at: DateTime<Utc>,
}

impl BotTask {
    /// Create a new BotTask instance (use insert() to save to database)
    pub fn new(
        untis_school: String,
        task_name: String,
        untis_login: String,
        untis_password: String,
        notification_chat_id: i64,
        status_chat_id: i64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            status_message_id: None,
            target_class_name: None,
            untis_school,
            task_name,
            untis_login,
            untis_password,
            updated_at: Utc::now(),
            notification_chat_id,
            notification_thread_id: None,
            status_chat_id,
            status_thread_id: None,
            timezone: "Europe/Berlin".to_string(),
        }
    }

    pub fn bind_lesson_to_self(&self, lesson: UnownedLesson) -> NewLesson {
        NewLesson {
            bot_state: self.id,
            lesson_id: lesson.lesson_id,
            date: lesson.date,
            end_time: lesson.end_time,
            lesson_type: lesson.lesson_type,
            start_time: lesson.start_time,
            subst_text: lesson.subst_text,
            lesson_code: lesson.lesson_code,
            classes: lesson.classes,
            rooms: lesson.rooms,
            subjects: lesson.subjects,
            teachers: lesson.teachers,
        }
    }

    /// Insert this bot state into the database, returning the inserted row.
    pub fn insert(&self) -> eyre::Result<BotTask> {
        use crate::diesel_impl::global_pool;
        use crate::schema::bot_states;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        let new_state = NewBotTask {
            id: self.id,
            status_message_id: self.status_message_id,
            target_class_name: self.target_class_name.clone(),
            untis_school: self.untis_school.clone(),
            task_name: self.task_name.clone(),
            untis_login: self.untis_login.clone(),
            untis_password: self.untis_password.clone(),
            updated_at: self.updated_at,
            notification_chat_id: self.notification_chat_id,
            notification_thread_id: self.notification_thread_id,
            status_chat_id: self.status_chat_id,
            status_thread_id: self.status_thread_id,
            timezone: self.timezone.clone(),
        };

        diesel::insert_into(bot_states::table)
            .values(&new_state)
            .returning(BotTask::as_returning())
            .get_result(&mut conn)
            .map_err(|e| eyre::eyre!("failed to insert bot_state: {e}"))
    }

    /// Reload this bot state from the database.
    pub fn reload(&mut self) -> eyre::Result<()> {
        use crate::diesel_impl::global_pool;
        use crate::schema::bot_states;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        let updated = bot_states::table
            .filter(bot_states::id.eq(self.id))
            .select(BotTask::as_select())
            .first(&mut conn)
            .map_err(|e| eyre::eyre!("failed to reload bot_state: {e}"))?;

        *self = updated;
        Ok(())
    }

    /// Update this bot state in the database using changeset.
    /// Returns the updated state and also updates self.
    pub fn update(&mut self, changeset: BotTaskChangeset) -> eyre::Result<()> {
        use crate::diesel_impl::global_pool;
        use crate::schema::bot_states;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        let updated = diesel::update(bot_states::table.filter(bot_states::id.eq(self.id)))
            .set(&changeset)
            .returning(BotTask::as_returning())
            .get_result(&mut conn)
            .map_err(|e| eyre::eyre!("failed to update bot_state: {e}"))?;

        *self = updated;
        Ok(())
    }

    /// Update this bot state with optimistic locking.
    /// Only updates if the current updated_at matches expected_updated_at.
    /// Returns Ok(true) if successful, Ok(false) if version mismatch, Err on other errors.
    /// On success, updates self with the new state.
    pub fn update_optimistic(
        &mut self,
        expected_updated_at: DateTime<Utc>,
        changeset: BotTaskChangeset,
    ) -> eyre::Result<bool> {
        use crate::diesel_impl::global_pool;
        use crate::schema::bot_states;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        let result = diesel::update(
            bot_states::table
                .filter(bot_states::id.eq(self.id))
                .filter(bot_states::updated_at.eq(expected_updated_at)),
        )
        .set(&changeset)
        .returning(BotTask::as_returning())
        .get_result(&mut conn)
        .optional()
        .map_err(|e| eyre::eyre!("failed to update bot_state with optimistic lock: {e}"))?;

        if let Some(updated) = result {
            *self = updated;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Delete this bot state from the database.
    pub fn delete(self) -> eyre::Result<usize> {
        use crate::diesel_impl::global_pool;
        use crate::schema::bot_states;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        diesel::delete(bot_states::table.filter(bot_states::id.eq(self.id)))
            .execute(&mut conn)
            .map_err(|e| eyre::eyre!("failed to delete bot_state: {e}"))
    }

    // Static methods for querying multiple states

    /// Retrieve a bot state by ID.
    pub fn get_by_id(state_id: Uuid) -> eyre::Result<Option<BotTask>> {
        use crate::diesel_impl::global_pool;
        use crate::schema::bot_states;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        bot_states::table
            .filter(bot_states::id.eq(state_id))
            .select(BotTask::as_select())
            .first(&mut conn)
            .optional()
            .map_err(|e| eyre::eyre!("failed to get bot_state: {e}"))
    }

    /// Get all bot states ordered by updated_at desc.
    pub fn get_all() -> eyre::Result<Vec<BotTask>> {
        use crate::diesel_impl::global_pool;
        use crate::schema::bot_states;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        bot_states::table
            .select(BotTask::as_select())
            .order((bot_states::updated_at.desc(), bot_states::id.asc()))
            .load(&mut conn)
            .map_err(|e| eyre::eyre!("failed to get all bot_states: {e}"))
    }

    // Lesson operations for this bot state

    /// Get all lessons for this bot state.
    pub fn get_lessons(&self) -> eyre::Result<Vec<Lesson>> {
        use crate::diesel_impl::global_pool;
        use crate::schema::lessons;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        lessons::table
            .filter(lessons::bot_state.eq(self.id))
            .select(Lesson::as_select())
            .order((lessons::date.asc(), lessons::start_time.asc()))
            .load(&mut conn)
            .map_err(|e| eyre::eyre!("failed to get lessons for bot_state: {e}"))
    }

    /// Insert a lesson associated with this bot state.
    pub fn insert_lesson(&self, mut new_lesson: NewLesson) -> eyre::Result<Lesson> {
        use crate::diesel_impl::global_pool;
        use crate::schema::lessons;
        use diesel::prelude::*;

        // Ensure the lesson is associated with this bot state
        new_lesson.bot_state = self.id;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        diesel::insert_into(lessons::table)
            .values(&new_lesson)
            .get_result(&mut conn)
            .map_err(|e| eyre::eyre!("failed to insert lesson: {e}"))
    }

    /// Insert multiple lessons associated with this bot state in a batch.
    pub fn insert_lessons_batch(
        &self,
        mut new_lessons: Vec<NewLesson>,
    ) -> eyre::Result<Vec<Lesson>> {
        use crate::diesel_impl::global_pool;
        use crate::schema::lessons;
        use diesel::prelude::*;

        // Ensure all lessons are associated with this bot state
        for lesson in &mut new_lessons {
            lesson.bot_state = self.id;
        }

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        diesel::insert_into(lessons::table)
            .values(&new_lessons)
            .get_results(&mut conn)
            .map_err(|e| eyre::eyre!("failed to insert lessons batch: {e}"))
    }

    /// Delete all lessons associated with this bot state.
    pub fn delete_lessons(&self) -> eyre::Result<usize> {
        use crate::diesel_impl::global_pool;
        use crate::schema::lessons;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        diesel::delete(lessons::table.filter(lessons::bot_state.eq(self.id)))
            .execute(&mut conn)
            .map_err(|e| eyre::eyre!("failed to delete lessons for bot_state: {e}"))
    }
}

/// Represents the status of a lesson (regular, cancelled, etc.)
#[derive(
    DbEnum,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    strum::Display,
    Default,
)]
#[ExistingTypePath = "crate::schema::sql_types::LessonCode"]
#[DbValueStyle = "snake_case"]
#[serde(rename_all = "lowercase")]
pub enum LessonCode {
    #[default]
    Regular,
    Irregular,
    Cancelled,
}

#[derive(
    Debug,
    Clone,
    Queryable,
    Identifiable,
    Selectable,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    restructed::Models,
)]
#[view(
    UnownedLesson,
    derive(PartialEq, Eq)
    omit(bot_state)
)]
#[view(
    NewLesson,
    derive(Insertable, Serialize, Deserialize),
    attributes_with = "deriveless"
)]
#[diesel(table_name = lessons)]
#[diesel(primary_key(lesson_id))]
pub struct Lesson {
    pub bot_state: Uuid,
    pub lesson_id: i64,
    pub date: NaiveDate,
    pub end_time: NaiveTime,
    pub lesson_type: String,
    pub start_time: NaiveTime,
    pub subst_text: Option<String>,
    pub lesson_code: LessonCode,
    pub classes: Vec<String>,
    pub rooms: Vec<String>,
    pub subjects: Vec<String>,
    pub teachers: Vec<String>,
}

impl Lesson {
    /// Get a lesson by ID.
    pub fn get_by_id(lesson_id: i64) -> eyre::Result<Option<Lesson>> {
        use crate::diesel_impl::global_pool;
        use crate::schema::lessons;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        lessons::table
            .filter(lessons::lesson_id.eq(lesson_id))
            .select(Lesson::as_select())
            .first(&mut conn)
            .optional()
            .map_err(|e| eyre::eyre!("failed to get lesson: {e}"))
    }

    pub fn get_owned_by_task_id(task_id: Uuid) -> eyre::Result<Vec<Lesson>> {
        use crate::diesel_impl::global_pool;
        use crate::schema::lessons;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        lessons::table
            .filter(lessons::bot_state.eq(task_id))
            .select(Lesson::as_select())
            .order((
                lessons::date.asc(),
                lessons::start_time.asc(),
                lessons::lesson_id.asc(),
            ))
            .load(&mut conn)
            .map_err(|e| eyre::eyre!("failed to get lessons for bot_state: {e}"))
    }

    /// Get all lessons ordered by date, start time.
    pub fn get_all() -> eyre::Result<Vec<Lesson>> {
        use crate::diesel_impl::global_pool;
        use crate::schema::lessons;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        lessons::table
            .select(Lesson::as_select())
            .order((
                lessons::date.asc(),
                lessons::start_time.asc(),
                lessons::lesson_id.asc(),
            ))
            .load(&mut conn)
            .map_err(|e| eyre::eyre!("failed to get all lessons: {e}"))
    }

    /// Delete this lesson from the database.
    pub fn delete(self) -> eyre::Result<usize> {
        use crate::diesel_impl::global_pool;
        use crate::schema::lessons;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        diesel::delete(lessons::table.filter(lessons::lesson_id.eq(self.lesson_id)))
            .execute(&mut conn)
            .map_err(|e| eyre::eyre!("failed to delete lesson: {e}"))
    }
}

/// Delete all lessons with date before the specified date
pub fn delete_lessons_before(before_date: chrono::NaiveDate) -> eyre::Result<usize> {
    use crate::diesel_impl::global_pool;
    use crate::schema::lessons;
    use diesel::prelude::*;

    let pool = global_pool()?;
    let mut conn = pool.get()?;

    diesel::delete(lessons::table.filter(lessons::date.lt(before_date)))
        .execute(&mut conn)
        .map_err(|e| eyre::eyre!("failed to delete old lessons: {e}"))
}

#[derive(
    Debug, Clone, Queryable, Identifiable, Selectable, Serialize, Deserialize, restructed::Models,
)]
#[view(
    NewLogEntry,
    derive(Insertable, Serialize, Deserialize),
    attributes_with = "deriveless",
    omit(ts)
)]
#[diesel(table_name = logs)]
pub struct LogEntry {
    pub id: Uuid,
    pub ts: DateTime<Utc>,
    pub level: String,
    pub target: Option<String>,
    pub message: String,
    pub fields: Option<serde_json::Value>,
    pub file: Option<String>,
    pub line: Option<i32>,
}

impl NewLogEntry {
    /// Insert a new log entry and return the inserted row.
    pub fn insert(self) -> eyre::Result<LogEntry> {
        use crate::diesel_impl::global_pool;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        diesel::insert_into(logs::table)
            .values(&self)
            .returning(LogEntry::as_returning())
            .get_result(&mut conn)
            .map_err(|e| eyre::eyre!("failed to insert log entry: {e}"))
    }
}

impl LogEntry {
    /// Fetch latest N log entries.
    pub fn latest(limit: i64) -> eyre::Result<Vec<LogEntry>> {
        use crate::diesel_impl::global_pool;
        use diesel::prelude::*;

        let pool = global_pool()?;
        let mut conn = pool.get()?;

        logs::table
            .order(logs::ts.desc())
            .limit(limit)
            .select(LogEntry::as_select())
            .load(&mut conn)
            .map_err(|e| eyre::eyre!("failed to fetch log entries: {e}"))
    }
}

/// Delete log entries with timestamp before provided UTC time.
pub fn delete_logs_before(before: DateTime<Utc>) -> eyre::Result<usize> {
    use crate::diesel_impl::global_pool;
    use crate::schema::logs;
    use diesel::prelude::*;

    let pool = global_pool()?;
    let mut conn = pool.get()?;

    diesel::delete(logs::table.filter(logs::ts.lt(before)))
        .execute(&mut conn)
        .map_err(|e| eyre::eyre!("failed to delete old logs: {e}"))
}
