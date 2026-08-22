use crate::models::{iso_now, CheckInAnswer, CheckInV1, SettingsV1};
use rusqlite::{params, Connection, OptionalExtension};
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
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|error| format!("No se pudo proteger la base local: {error}"))?;
        }
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
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
                 CREATE INDEX IF NOT EXISTS checkins_status_idx ON checkins(status);",
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
                .execute("DELETE FROM app_state", [])
                .map_err(|error| error.to_string())?;
            transaction.commit().map_err(|error| error.to_string())?;
        }
        self.save_settings(&SettingsV1::default())?;
        self.set_value("study_started_at", &iso_now())?;
        self.set_value("selected_days", "7")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CheckInAnswer, CheckInV1};

    fn store() -> LocalStore {
        let directory = tempfile::tempdir().unwrap();
        // Keep the directory for the duration of the process; the connection owns the file.
        let path = directory.keep().join("enso-time.sqlite");
        LocalStore::open(&path).unwrap()
    }

    #[test]
    fn rejects_invalid_cadence() {
        let store = store();
        let mut settings = store.settings().unwrap();
        settings.cadence_minutes = 12;
        assert!(store.save_settings(&settings).is_err());
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
}
