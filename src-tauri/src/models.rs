use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const CATEGORIES: [&str; 9] = [
    "focus",
    "communication",
    "meetings",
    "admin",
    "learning",
    "creation",
    "personal",
    "break",
    "unknown",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkSchedule {
    pub start: String,
    pub end: String,
    pub days: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRule {
    pub id: String,
    pub pattern: String,
    pub category: String,
    pub activity_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsV1 {
    pub schema_version: u8,
    pub onboarding_completed: bool,
    pub locale: String,
    pub theme: String,
    pub timezone: String,
    pub cadence_minutes: u16,
    pub autostart: bool,
    pub capture_window_titles: bool,
    pub retention_days: Option<u16>,
    pub update_checks_enabled: bool,
    pub goals: Vec<String>,
    pub role: String,
    pub work_schedule: WorkSchedule,
    pub app_rules: Vec<AppRule>,
}

impl Default for SettingsV1 {
    fn default() -> Self {
        Self {
            schema_version: 1,
            onboarding_completed: false,
            locale: "es-419".into(),
            theme: "ink".into(),
            timezone: "America/Bogota".into(),
            cadence_minutes: 15,
            autostart: true,
            capture_window_titles: false,
            retention_days: Some(90),
            update_checks_enabled: false,
            goals: vec!["Recuperar tiempo para trabajo de mayor valor".into()],
            role: String::new(),
            work_schedule: WorkSchedule {
                start: "08:30".into(),
                end: "18:00".into(),
                days: vec![1, 2, 3, 4, 5],
            },
            app_rules: vec![],
        }
    }
}

impl SettingsV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("La versión de configuración no es compatible".into());
        }
        if !matches!(self.cadence_minutes, 15 | 30 | 60) {
            return Err("La cadencia debe ser 15, 30 o 60 minutos".into());
        }
        if !matches!(self.locale.as_str(), "es-419" | "en") {
            return Err("El idioma seleccionado no es compatible".into());
        }
        if !matches!(self.theme.as_str(), "ink" | "paper" | "system") {
            return Err("El tema seleccionado no es compatible".into());
        }
        if self.timezone.parse::<chrono_tz::Tz>().is_err() {
            return Err("La zona horaria no es válida".into());
        }
        if self.role.chars().count() > 200
            || self.goals.len() > 10
            || self.goals.iter().any(|goal| goal.chars().count() > 500)
        {
            return Err("El perfil o los objetivos exceden el tamaño permitido".into());
        }
        if self.work_schedule.days.iter().any(|day| *day > 6) {
            return Err("El horario contiene un día inválido".into());
        }
        validate_clock(&self.work_schedule.start)?;
        validate_clock(&self.work_schedule.end)?;
        if let Some(days) = self.retention_days {
            if !matches!(days, 30 | 90 | 365) {
                return Err("La retención debe ser 30, 90, 365 días o ilimitada".into());
            }
        }
        if self.app_rules.len() > 500 {
            return Err("Hay demasiadas reglas de aplicación".into());
        }
        for rule in &self.app_rules {
            if rule.id.chars().count() > 128
                || rule.pattern.trim().is_empty()
                || rule.pattern.chars().count() > 256
                || rule
                    .activity_label
                    .as_ref()
                    .is_some_and(|label| label.chars().count() > 200)
                || !is_category(&rule.category)
            {
                return Err("Hay una regla de aplicación inválida".into());
            }
        }
        Ok(())
    }
}

fn validate_clock(value: &str) -> Result<(), String> {
    let Some((hour, minute)) = value.split_once(':') else {
        return Err("El horario debe usar el formato HH:MM".into());
    };
    let hour = hour.parse::<u8>().map_err(|_| "Hora inválida")?;
    let minute = minute.parse::<u8>().map_err(|_| "Minuto inválido")?;
    if hour > 23 || minute > 59 {
        return Err("El horario debe usar el formato HH:MM".into());
    }
    Ok(())
}

pub fn is_category(value: &str) -> bool {
    CATEGORIES.contains(&value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureIssueCode {
    ServerUnavailable,
    WindowWatcherUnavailable,
    AfkWatcherUnavailable,
    WatcherConflict,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingStatus {
    pub running: bool,
    pub paused_until: Option<String>,
    pub server_port: Option<u16>,
    pub window_watcher: String,
    pub afk_watcher: String,
    pub last_event_at: Option<String>,
    pub pending_check_ins: usize,
    pub data_stays_local: bool,
    pub capture_issue: Option<CaptureIssueCode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineBlockV1 {
    pub id: String,
    pub start: String,
    pub end: String,
    pub duration_seconds: i64,
    pub app: String,
    pub title: Option<String>,
    pub category: String,
    pub confidence: f64,
    pub afk: bool,
    pub source: String,
    pub check_in_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckInV1 {
    pub id: String,
    pub interval_start: String,
    pub interval_end: String,
    pub reason: String,
    pub status: String,
    pub activity_label: Option<String>,
    pub category: Option<String>,
    pub project: Option<String>,
    pub repetitive: Option<bool>,
    pub rules_clear: Option<bool>,
    pub created_at: String,
    pub answered_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckInAnswer {
    pub id: String,
    pub activity_label: String,
    pub category: String,
    pub project: Option<String>,
    pub repetitive: bool,
    pub rules_clear: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricValue {
    pub label: String,
    pub value: f64,
    pub unit: String,
    pub delta: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationCandidateV1 {
    pub id: String,
    pub activity_label: String,
    pub estimated_weekly_hours: f64,
    pub weekly_frequency: f64,
    pub impact_score: i32,
    pub feasibility_score: i32,
    pub risk_score: i32,
    pub priority_score: i32,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticV1 {
    pub schema_version: u8,
    pub range_start: String,
    pub range_end: String,
    pub days_observed: f64,
    pub metrics: Vec<MetricValue>,
    pub recommendations: Vec<String>,
    pub candidates: Vec<AutomationCandidateV1>,
    pub prompt: String,
    pub preliminary: bool,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyProgress {
    pub started_at: String,
    pub selected_days: u16,
    pub observed_days: f64,
    pub tracked_hours: f64,
    pub answered_check_ins: usize,
    pub coverage_percent: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEnvelope {
    pub schema_version: u8,
    pub exported_at: String,
    pub settings: SettingsV1,
    pub check_ins: Vec<CheckInV1>,
    pub timeline: Vec<TimelineBlockV1>,
}

pub fn iso_now() -> String {
    Utc::now().to_rfc3339()
}

pub fn parse_utc(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&Utc))
        .map_err(|error| format!("Fecha inválida: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_issue_codes_have_stable_wire_values() {
        let cases = [
            (CaptureIssueCode::ServerUnavailable, "server-unavailable"),
            (
                CaptureIssueCode::WindowWatcherUnavailable,
                "window-watcher-unavailable",
            ),
            (
                CaptureIssueCode::AfkWatcherUnavailable,
                "afk-watcher-unavailable",
            ),
            (CaptureIssueCode::WatcherConflict, "watcher-conflict"),
        ];
        for (code, expected) in cases {
            assert_eq!(
                serde_json::to_string(&code).unwrap(),
                format!("\"{expected}\"")
            );
            assert_eq!(
                serde_json::from_str::<CaptureIssueCode>(&format!("\"{expected}\"")).unwrap(),
                code
            );
        }
    }
}
