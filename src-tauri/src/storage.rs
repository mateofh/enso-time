use crate::models::{
    automation_candidate_id, iso_now, parse_utc, CheckInAnswer, CheckInV1, SettingsV1,
    WorkflowIntakeV1,
};
use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;
use std::sync::Mutex;

pub struct LocalStore {
    connection: Mutex<Connection>,
}

impl LocalStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("No se pudo crear el directorio de datos local: {error}")
            })?;
        }
        let mut connection = Connection::open(path).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|error| format!("No se pudo proteger la base local: {error}"))?;
        }
        connection
            .execute_batch(
                "PRAGMA secure_delete=ON;
                 PRAGMA journal_mode=WAL;
                 PRAGMA foreign_keys=ON;
                 CREATE TABLE IF NOT EXISTS app_state (
                   key TEXT PRIMARY KEY,
                   value TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS checkins (
                   id TEXT PRIMARY KEY,
                   interval_start TEXT NOT NULL,
                   interval_end TEXT NOT NULL,
                   reason TEXT NOT NULL,
                   status TEXT NOT NULL,
                   activity_label TEXT,
                   category TEXT,
                   project TEXT,
                   repetitive INTEGER,
                   rules_clear INTEGER,
                   created_at TEXT NOT NULL,
                   answered_at TEXT,
                   UNIQUE(interval_start, interval_end)
                 );
                 CREATE INDEX IF NOT EXISTS checkins_status_idx ON checkins(status);
                 CREATE TABLE IF NOT EXISTS workflow_intakes (
                   id TEXT PRIMARY KEY,
                   candidate_id TEXT NOT NULL,
                   candidate_label TEXT NOT NULL,
                   trigger TEXT NOT NULL,
                   tools_json TEXT NOT NULL,
                   steps_json TEXT NOT NULL,
                   human_decisions TEXT NOT NULL,
                   inputs TEXT NOT NULL,
                   desired_output TEXT NOT NULL,
                   frequency TEXT NOT NULL,
                   minutes_per_run INTEGER NOT NULL CHECK(minutes_per_run BETWEEN 1 AND 1440),
                   pain_points TEXT NOT NULL,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS workflow_intakes_updated_idx
                   ON workflow_intakes(updated_at DESC);",
            )
            .map_err(|error| error.to_string())?;
        migrate_workflow_candidate_ids(&mut connection)?;
        connection
            .execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS workflow_intakes_candidate_unique
                   ON workflow_intakes(candidate_id)",
                [],
            )
            .map_err(|error| error.to_string())?;
        let store = Self {
            connection: Mutex::new(connection),
        };
        if store.get_value("settings")?.is_none() {
            store.save_settings(&SettingsV1::default())?;
        }
        if store.get_value("study_started_at")?.is_none() {
            store.set_value("study_started_at", &iso_now())?;
            store.set_value("selected_days", "7")?;
        }
        Ok(store)
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.connection
            .lock()
            .map_err(|_| "No se pudo abrir la base local".into())
    }

    pub fn get_value(&self, key: &str) -> Result<Option<String>, String> {
        self.connection()?
            .query_row("SELECT value FROM app_state WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|error| error.to_string())
    }

    pub fn set_value(&self, key: &str, value: &str) -> Result<(), String> {
        self.connection()?
            .execute(
                "INSERT INTO app_state(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn settings(&self) -> Result<SettingsV1, String> {
        let raw = self.get_value("settings")?.unwrap_or_else(|| "{}".into());
        serde_json::from_str(&raw)
            .map_err(|error| format!("La configuración local está dañada: {error}"))
    }

    pub fn save_settings(&self, settings: &SettingsV1) -> Result<(), String> {
        settings.validate()?;
        let raw = serde_json::to_string(settings).map_err(|error| error.to_string())?;
        self.set_value("settings", &raw)
    }

    pub fn paused_until(&self) -> Result<Option<String>, String> {
        self.get_value("paused_until")
    }

    pub fn set_paused_until(&self, value: Option<&str>) -> Result<(), String> {
        match value {
            Some(value) => self.set_value("paused_until", value),
            None => self
                .connection()?
                .execute("DELETE FROM app_state WHERE key = 'paused_until'", [])
                .map(|_| ())
                .map_err(|error| error.to_string()),
        }
    }

    pub fn create_checkin(&self, item: &CheckInV1) -> Result<bool, String> {
        self.connection()?
            .execute(
                "INSERT OR IGNORE INTO checkins
                 (id, interval_start, interval_end, reason, status, created_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    item.id,
                    item.interval_start,
                    item.interval_end,
                    item.reason,
                    item.status,
                    item.created_at
                ],
            )
            .map(|changed| changed > 0)
            .map_err(|error| error.to_string())
    }

    pub fn checkins(&self) -> Result<Vec<CheckInV1>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, interval_start, interval_end, reason, status, activity_label, category,
                    project, repetitive, rules_clear, created_at, answered_at
             FROM checkins ORDER BY interval_start DESC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok(CheckInV1 {
                    id: row.get(0)?,
                    interval_start: row.get(1)?,
                    interval_end: row.get(2)?,
                    reason: row.get(3)?,
                    status: row.get(4)?,
                    activity_label: row.get(5)?,
                    category: row.get(6)?,
                    project: row.get(7)?,
                    repetitive: row.get::<_, Option<i64>>(8)?.map(|value| value != 0),
                    rules_clear: row.get::<_, Option<i64>>(9)?.map(|value| value != 0),
                    created_at: row.get(10)?,
                    answered_at: row.get(11)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn checkins_between(&self, from: &str, to: &str) -> Result<Vec<CheckInV1>, String> {
        let from = crate::models::parse_utc(from)?;
        let to = crate::models::parse_utc(to)?;
        Ok(self
            .checkins()?
            .into_iter()
            .filter(|item| {
                let Ok(start) = crate::models::parse_utc(&item.interval_start) else {
                    return false;
                };
                let Ok(end) = crate::models::parse_utc(&item.interval_end) else {
                    return false;
                };
                end > from && start < to
            })
            .collect())
    }

    pub fn answer_checkin(&self, answer: &CheckInAnswer) -> Result<CheckInV1, String> {
        if answer.activity_label.trim().is_empty() {
            return Err("Describe brevemente qué estabas haciendo".into());
        }
        if answer.id.chars().count() > 128
            || answer.activity_label.chars().count() > 500
            || answer
                .project
                .as_ref()
                .is_some_and(|project| project.chars().count() > 300)
        {
            return Err("La respuesta del check-in excede el tamaño permitido".into());
        }
        if !crate::models::is_category(&answer.category) {
            return Err("La categoría del check-in no es válida".into());
        }
        let answered_at = iso_now();
        let changed = self
            .connection()?
            .execute(
                "UPDATE checkins SET status = 'answered', activity_label = ?2, category = ?3,
             project = ?4, repetitive = ?5, rules_clear = ?6, answered_at = ?7 WHERE id = ?1",
                params![
                    answer.id,
                    answer.activity_label,
                    answer.category,
                    answer.project,
                    answer.repetitive as i32,
                    answer.rules_clear as i32,
                    answered_at
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed == 0 {
            return Err("El check-in ya no existe".into());
        }
        self.checkins()?
            .into_iter()
            .find(|item| item.id == answer.id)
            .ok_or_else(|| "No se pudo leer el check-in".into())
    }

    pub fn pending_count(&self) -> Result<usize, String> {
        self.connection()?
            .query_row(
                "SELECT COUNT(*) FROM checkins WHERE status = 'pending'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value as usize)
            .map_err(|error| error.to_string())
    }

    pub fn prune_checkins(&self, retention_days: Option<u16>) -> Result<usize, String> {
        let Some(days) = retention_days else {
            return Ok(0);
        };
        let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(days));
        self.connection()?
            .execute(
                "DELETE FROM checkins WHERE interval_end < ?1",
                [cutoff.to_rfc3339()],
            )
            .map_err(|error| error.to_string())
    }

    pub fn list_workflow_intakes(&self) -> Result<Vec<WorkflowIntakeV1>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, candidate_id, candidate_label, trigger, tools_json, steps_json,
                        human_decisions, inputs, desired_output, frequency, minutes_per_run,
                        pain_points, created_at, updated_at
                 FROM workflow_intakes
                 ORDER BY updated_at DESC, id ASC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], workflow_intake_from_row)
            .map_err(|error| error.to_string())?;
        let items = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Los intakes locales están dañados: {error}"))?;
        for item in &items {
            item.validate()
                .map_err(|error| format!("Un intake local está dañado: {error}"))?;
        }
        Ok(items)
    }

    pub fn save_workflow_intake(
        &self,
        intake: WorkflowIntakeV1,
    ) -> Result<WorkflowIntakeV1, String> {
        let intake = intake.normalize_and_validate()?;
        let tools_json = serde_json::to_string(&intake.tools).map_err(|error| error.to_string())?;
        let steps_json = serde_json::to_string(&intake.steps).map_err(|error| error.to_string())?;
        let stored = {
            let connection = self.connection()?;
            let exists = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM workflow_intakes WHERE candidate_id = ?1)",
                    [&intake.candidate_id],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(|error| error.to_string())?;
            if !exists {
                let count = connection
                    .query_row("SELECT COUNT(*) FROM workflow_intakes", [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .map_err(|error| error.to_string())?;
                if count >= 500 {
                    return Err("No se pueden guardar más de 500 intakes de workflow".into());
                }
            }
            connection
                .execute(
                    "INSERT INTO workflow_intakes
                     (id, candidate_id, candidate_label, trigger, tools_json, steps_json,
                      human_decisions, inputs, desired_output, frequency, minutes_per_run,
                      pain_points, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                     ON CONFLICT(candidate_id) DO UPDATE SET
                       candidate_label = excluded.candidate_label,
                       trigger = excluded.trigger,
                       tools_json = excluded.tools_json,
                       steps_json = excluded.steps_json,
                       human_decisions = excluded.human_decisions,
                       inputs = excluded.inputs,
                       desired_output = excluded.desired_output,
                       frequency = excluded.frequency,
                       minutes_per_run = excluded.minutes_per_run,
                       pain_points = excluded.pain_points,
                       updated_at = excluded.updated_at
                     WHERE julianday(excluded.updated_at) >= julianday(workflow_intakes.updated_at)",
                    params![
                        intake.id,
                        intake.candidate_id,
                        intake.candidate_label,
                        intake.trigger,
                        tools_json,
                        steps_json,
                        intake.human_decisions,
                        intake.inputs,
                        intake.desired_output,
                        intake.frequency,
                        intake.minutes_per_run,
                        intake.pain_points,
                        intake.created_at,
                        intake.updated_at,
                    ],
                )
                .map_err(|error| error.to_string())?;
            connection
                .query_row(
                    "SELECT id, candidate_id, candidate_label, trigger, tools_json, steps_json,
                            human_decisions, inputs, desired_output, frequency, minutes_per_run,
                            pain_points, created_at, updated_at
                     FROM workflow_intakes WHERE candidate_id = ?1",
                    [&intake.candidate_id],
                    workflow_intake_from_row,
                )
                .map_err(|error| error.to_string())?
        };
        stored
            .validate()
            .map_err(|error| format!("El intake guardado está dañado: {error}"))?;
        Ok(stored)
    }

    pub fn delete_all(&self) -> Result<(), String> {
        {
            let mut connection = self.connection()?;
            let transaction = connection
                .transaction()
                .map_err(|error| error.to_string())?;
            transaction
                .execute("DELETE FROM checkins", [])
                .map_err(|error| error.to_string())?;
            transaction
                .execute("DELETE FROM workflow_intakes", [])
                .map_err(|error| error.to_string())?;
            transaction
                .execute("DELETE FROM app_state", [])
                .map_err(|error| error.to_string())?;
            transaction.commit().map_err(|error| error.to_string())?;
        }
        self.save_settings(&SettingsV1::default())?;
        self.set_value("study_started_at", &iso_now())?;
        self.set_value("selected_days", "7")?;
        self.connection()?
            .execute_batch(
                "PRAGMA wal_checkpoint(TRUNCATE);
                 VACUUM;
                 PRAGMA wal_checkpoint(TRUNCATE);
                 PRAGMA journal_mode=DELETE;",
            )
            .map_err(|error| format!("Los datos se borraron, pero la compactación falló: {error}"))
    }
}

#[derive(Debug)]
struct WorkflowCandidateMigrationRow {
    rowid: i64,
    intake_id: String,
    candidate_id: String,
    target_candidate_id: String,
    updated_at: chrono::DateTime<chrono::Utc>,
}

fn migrate_workflow_candidate_ids(connection: &mut Connection) -> Result<(), String> {
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let mut rows = {
        let mut statement = transaction
            .prepare(
                "SELECT rowid, id, candidate_id, candidate_label, updated_at
                 FROM workflow_intakes",
            )
            .map_err(|error| error.to_string())?;
        let values = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        values
    }
    .into_iter()
    .map(
        |(rowid, intake_id, candidate_id, candidate_label, updated_at)| {
            let target_candidate_id = automation_candidate_id(&candidate_label);
            let updated_at = parse_utc(&updated_at).map_err(|error| {
                format!("El intake {intake_id} tiene updatedAt inválido: {error}")
            })?;
            Ok(WorkflowCandidateMigrationRow {
                rowid,
                intake_id,
                candidate_id,
                target_candidate_id,
                updated_at,
            })
        },
    )
    .collect::<Result<Vec<_>, String>>()?;

    rows.sort_by(|left, right| {
        left.target_candidate_id
            .cmp(&right.target_candidate_id)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| left.intake_id.cmp(&right.intake_id))
            .then_with(|| left.rowid.cmp(&right.rowid))
    });

    let mut winners = Vec::new();
    let mut loser_rowids = Vec::new();
    for row in rows {
        if winners
            .last()
            .is_some_and(|winner: &WorkflowCandidateMigrationRow| {
                winner.target_candidate_id == row.target_candidate_id
            })
        {
            loser_rowids.push(row.rowid);
        } else {
            winners.push(row);
        }
    }

    let requires_migration = !loser_rowids.is_empty()
        || winners
            .iter()
            .any(|row| row.candidate_id != row.target_candidate_id);
    if requires_migration {
        // Previous releases created this unique index before candidate IDs were
        // canonical. Rebuild it inside the same transaction so converging IDs and
        // even legacy ID swaps cannot fail halfway through the migration.
        transaction
            .execute("DROP INDEX IF EXISTS workflow_intakes_candidate_unique", [])
            .map_err(|error| error.to_string())?;
        for rowid in loser_rowids {
            transaction
                .execute("DELETE FROM workflow_intakes WHERE rowid = ?1", [rowid])
                .map_err(|error| error.to_string())?;
        }

        for row in &winners {
            transaction
                .execute(
                    "UPDATE workflow_intakes SET candidate_id = ?1 WHERE rowid = ?2",
                    params![row.target_candidate_id, row.rowid],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction
            .execute(
                "CREATE UNIQUE INDEX workflow_intakes_candidate_unique
                   ON workflow_intakes(candidate_id)",
                [],
            )
            .map_err(|error| error.to_string())?;
    }

    transaction.commit().map_err(|error| error.to_string())
}

fn workflow_intake_from_row(row: &Row<'_>) -> rusqlite::Result<WorkflowIntakeV1> {
    let tools_json: String = row.get(4)?;
    let steps_json: String = row.get(5)?;
    let tools = serde_json::from_str(&tools_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, Type::Text, Box::new(error))
    })?;
    let steps = serde_json::from_str(&steps_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, Type::Text, Box::new(error))
    })?;
    Ok(WorkflowIntakeV1 {
        id: row.get(0)?,
        candidate_id: row.get(1)?,
        candidate_label: row.get(2)?,
        trigger: row.get(3)?,
        tools,
        steps,
        human_decisions: row.get(6)?,
        inputs: row.get(7)?,
        desired_output: row.get(8)?,
        frequency: row.get(9)?,
        minutes_per_run: row.get(10)?,
        pain_points: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::create_diagnostic;
    use crate::models::{CheckInAnswer, CheckInV1, SettingsV1, WorkflowIntakeV1};
    use std::sync::{Arc, Barrier};

    fn store() -> LocalStore {
        let directory = tempfile::tempdir().unwrap();
        // Keep the directory for the duration of the process; the connection owns the file.
        let path = directory.keep().join("enso-time.sqlite");
        LocalStore::open(&path).unwrap()
    }

    fn workflow(id: &str, label: &str, updated_at: &str) -> WorkflowIntakeV1 {
        WorkflowIntakeV1 {
            id: id.into(),
            candidate_id: "candidate-report".into(),
            candidate_label: label.into(),
            trigger: "Cada viernes".into(),
            tools: vec!["Sheets".into(), "Email".into()],
            steps: vec!["Extraer datos".into(), "Revisar excepciones".into()],
            human_decisions: "Aprobar diferencias".into(),
            inputs: "Ventas semanales".into(),
            desired_output: "Reporte validado".into(),
            frequency: "Semanal".into(),
            minutes_per_run: 45,
            pain_points: "Copiar y pegar".into(),
            created_at: "2025-01-01T10:00:00Z".into(),
            updated_at: updated_at.into(),
        }
    }

    #[test]
    fn rejects_invalid_cadence() {
        let store = store();
        let mut settings = store.settings().unwrap();
        settings.cadence_minutes = 12;
        assert!(store.save_settings(&settings).is_err());
    }

    #[test]
    fn secure_delete_is_enabled_for_local_records() {
        let store = store();
        let enabled: i64 = store
            .connection()
            .unwrap()
            .query_row("PRAGMA secure_delete", [], |row| row.get(0))
            .unwrap();
        assert_eq!(enabled, 1);
    }

    #[test]
    fn delete_all_compacts_and_removes_local_wal_sidecars() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("enso-time.sqlite");
        let store = LocalStore::open(&path).unwrap();
        store
            .save_workflow_intake(workflow(
                "private-workflow",
                "private-workflow-marker",
                "2025-01-02T10:00:00Z",
            ))
            .unwrap();
        store.delete_all().unwrap();
        assert!(store.list_workflow_intakes().unwrap().is_empty());
        assert!(!path.with_file_name("enso-time.sqlite-wal").exists());
        assert!(!path.with_file_name("enso-time.sqlite-shm").exists());
        let bytes = std::fs::read(path).unwrap();
        assert!(!bytes
            .windows(b"private-workflow-marker".len())
            .any(|window| window == b"private-workflow-marker"));
    }

    #[test]
    fn checkin_can_only_be_answered_with_valid_context() {
        let store = store();
        let item = CheckInV1 {
            id: "check-1".into(),
            interval_start: "2026-08-21T12:00:00Z".into(),
            interval_end: "2026-08-21T12:15:00Z".into(),
            reason: "unknown".into(),
            status: "pending".into(),
            activity_label: None,
            category: None,
            project: None,
            repetitive: None,
            rules_clear: None,
            created_at: iso_now(),
            answered_at: None,
        };
        assert!(store.create_checkin(&item).unwrap());
        let answer = CheckInAnswer {
            id: item.id,
            activity_label: "Conciliación".into(),
            category: "admin".into(),
            project: None,
            repetitive: true,
            rules_clear: true,
        };
        assert_eq!(store.answer_checkin(&answer).unwrap().status, "answered");
        assert_eq!(store.pending_count().unwrap(), 0);
    }

    #[test]
    fn workflow_intake_crud_normalizes_and_preserves_creation_time() {
        let store = store();
        let mut first = workflow(
            "workflow-1",
            "  Preparar   reporte ",
            "2025-01-02T10:00:00Z",
        );
        first.tools.push("sheets".into());
        let stored = store.save_workflow_intake(first).unwrap();
        assert_eq!(stored.candidate_label, "Preparar reporte");
        assert_eq!(stored.tools, ["Sheets", "Email"]);
        assert_eq!(store.list_workflow_intakes().unwrap(), [stored.clone()]);
        assert!(!store.settings().unwrap().onboarding_completed);

        let mut update = workflow(
            "workflow-replacement-id",
            "Preparar reporte ejecutivo",
            "2025-01-03T10:00:00Z",
        );
        update.created_at = "2025-01-02T10:00:00Z".into();
        let updated = store.save_workflow_intake(update).unwrap();
        assert_eq!(updated.id, "workflow-1");
        assert_eq!(updated.candidate_label, "Preparar reporte ejecutivo");
        assert_eq!(updated.created_at, stored.created_at);

        let stale = workflow(
            "workflow-stale-id",
            "Valor obsoleto",
            "2025-01-02T12:00:00Z",
        );
        let unchanged = store.save_workflow_intake(stale).unwrap();
        assert_eq!(unchanged.candidate_label, "Preparar reporte ejecutivo");
    }

    #[test]
    fn concurrent_workflow_upserts_keep_the_newest_timestamp() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(LocalStore::open(&directory.path().join("enso-time.sqlite")).unwrap());
        let barrier = Arc::new(Barrier::new(3));
        let mut threads = Vec::new();
        for (label, updated_at) in [
            ("Versión anterior", "2025-01-02T10:00:00Z"),
            ("Versión final", "2025-01-03T10:00:00Z"),
        ] {
            let thread_store = Arc::clone(&store);
            let thread_barrier = Arc::clone(&barrier);
            threads.push(std::thread::spawn(move || {
                thread_barrier.wait();
                thread_store
                    .save_workflow_intake(workflow("workflow-race", label, updated_at))
                    .unwrap();
            }));
        }
        barrier.wait();
        for thread in threads {
            thread.join().unwrap();
        }
        let items = store.list_workflow_intakes().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].candidate_label, "Versión final");
        assert_eq!(items[0].updated_at, "2025-01-03T10:00:00+00:00");
    }

    #[test]
    fn opening_a_legacy_database_adds_workflows_without_rewriting_existing_state() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("enso-time.sqlite");
        let legacy = Connection::open(&path).unwrap();
        legacy
            .execute_batch(
                "CREATE TABLE app_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE checkins (id TEXT PRIMARY KEY, status TEXT NOT NULL);
                 INSERT INTO app_state(key, value) VALUES('legacy-marker', 'preserved');",
            )
            .unwrap();
        drop(legacy);

        let store = LocalStore::open(&path).unwrap();
        assert_eq!(
            store.get_value("legacy-marker").unwrap().as_deref(),
            Some("preserved")
        );
        store
            .save_workflow_intake(workflow(
                "workflow-migrated",
                "Migrado",
                "2025-01-02T10:00:00Z",
            ))
            .unwrap();
        assert_eq!(store.list_workflow_intakes().unwrap().len(), 1);
    }

    #[test]
    fn opening_migrates_legacy_candidate_ids_and_keeps_newest_canonical_variant() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("enso-time.sqlite");
        {
            let store = LocalStore::open(&path).unwrap();

            let mut older = workflow("older-intake", "Preparar reporte", "2025-01-02T10:00:00Z");
            older.candidate_id = "candidate-preparar-reporte".into();
            older.pain_points = "legacy-older-marker".into();
            store.save_workflow_intake(older).unwrap();

            let mut newest = workflow("newest-intake", "preparar reporte", "2025-01-03T10:00:00Z");
            newest.candidate_id = "candidate-another-legacy-id".into();
            newest.created_at = "2025-01-02T09:00:00Z".into();
            newest.pain_points = "canonical-newest-marker".into();
            store.save_workflow_intake(newest).unwrap();

            let mut tie_loser = workflow("tie-z", "enviar informe 🚀", "2025-01-04T10:00:00Z");
            tie_loser.candidate_id = "candidate-tie-z-legacy".into();
            tie_loser.pain_points = "tie-z-marker".into();
            store.save_workflow_intake(tie_loser).unwrap();

            let mut tie_winner = workflow("tie-a", "Enviar informe 🚀", "2025-01-04T10:00:00Z");
            tie_winner.candidate_id = "candidate-tie-a-legacy".into();
            tie_winner.pain_points = "tie-a-marker".into();
            store.save_workflow_intake(tie_winner).unwrap();
        }

        let store = LocalStore::open(&path).unwrap();
        let items = store.list_workflow_intakes().unwrap();
        assert_eq!(items.len(), 2);

        let migrated = items
            .iter()
            .find(|item| item.candidate_id == automation_candidate_id("Preparar reporte"))
            .unwrap();
        assert_eq!(migrated.id, "newest-intake");
        assert_eq!(migrated.candidate_label, "preparar reporte");
        assert_eq!(migrated.created_at, "2025-01-02T09:00:00+00:00");
        assert_eq!(migrated.pain_points, "canonical-newest-marker");

        let tied = items
            .iter()
            .find(|item| item.candidate_id == automation_candidate_id("Enviar informe 🚀"))
            .unwrap();
        assert_eq!(tied.id, "tie-a");
        assert_eq!(tied.pain_points, "tie-a-marker");

        let diagnostic = create_diagnostic(
            &[],
            &[
                CheckInV1 {
                    id: "migration-checkin-1".into(),
                    interval_start: "2025-01-01T10:00:00Z".into(),
                    interval_end: "2025-01-01T10:15:00Z".into(),
                    reason: "ambiguous".into(),
                    status: "answered".into(),
                    activity_label: Some("Preparar reporte".into()),
                    category: Some("admin".into()),
                    project: None,
                    repetitive: Some(true),
                    rules_clear: Some(true),
                    created_at: "2025-01-01T10:15:00Z".into(),
                    answered_at: Some("2025-01-01T10:16:00Z".into()),
                },
                CheckInV1 {
                    id: "migration-checkin-2".into(),
                    interval_start: "2025-01-02T10:00:00Z".into(),
                    interval_end: "2025-01-02T10:15:00Z".into(),
                    reason: "ambiguous".into(),
                    status: "answered".into(),
                    activity_label: Some("preparar reporte".into()),
                    category: Some("admin".into()),
                    project: None,
                    repetitive: Some(true),
                    rules_clear: Some(true),
                    created_at: "2025-01-02T10:15:00Z".into(),
                    answered_at: Some("2025-01-02T10:16:00Z".into()),
                },
            ],
            7.0,
            &SettingsV1::default(),
            &items,
        );
        assert!(diagnostic.prompt.contains("canonical-newest-marker"));
        assert!(!diagnostic.prompt.contains("legacy-older-marker"));

        drop(store);
        let reopened = LocalStore::open(&path).unwrap();
        assert_eq!(reopened.list_workflow_intakes().unwrap(), items);
    }
}
