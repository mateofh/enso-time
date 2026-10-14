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
pub struct SourcePolicyV2 {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub cloud_allowed: bool,
}

impl Default for SourcePolicyV2 {
    fn default() -> Self {
        Self {
            enabled: false,
            cloud_allowed: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepCaptureSettingsV2 {
    #[serde(default)]
    pub browser: SourcePolicyV2,
    #[serde(default)]
    pub editor: SourcePolicyV2,
    #[serde(default)]
    pub shell: SourcePolicyV2,
    #[serde(default)]
    pub screenshots: SourcePolicyV2,
    #[serde(default = "default_screenshot_interval")]
    pub screenshot_interval_minutes: u16,
    #[serde(default)]
    pub browser_text_allowlist: Vec<String>,
    #[serde(default)]
    pub excluded_apps: Vec<String>,
    #[serde(default)]
    pub excluded_sites: Vec<String>,
}

fn default_screenshot_interval() -> u16 {
    15
}

impl Default for DeepCaptureSettingsV2 {
    fn default() -> Self {
        Self {
            browser: SourcePolicyV2::default(),
            editor: SourcePolicyV2::default(),
            shell: SourcePolicyV2::default(),
            screenshots: SourcePolicyV2::default(),
            screenshot_interval_minutes: default_screenshot_interval(),
            browser_text_allowlist: Vec::new(),
            excluded_apps: vec![
                "1Password".into(),
                "Bitwarden".into(),
                "Passwords".into(),
                "Keychain Access".into(),
            ],
            excluded_sites: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettingsV2 {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub analyst_model: Option<String>,
    #[serde(default)]
    pub evaluator_model: Option<String>,
    #[serde(default)]
    pub monthly_budget_usd: Option<f64>,
    #[serde(default)]
    pub weekly_enabled: bool,
    #[serde(default = "default_weekly_day")]
    pub weekly_day: u8,
    #[serde(default = "default_weekly_hour")]
    pub weekly_hour: u8,
}

fn default_weekly_day() -> u8 {
    1
}

fn default_weekly_hour() -> u8 {
    8
}

impl Default for AiSettingsV2 {
    fn default() -> Self {
        Self {
            enabled: false,
            analyst_model: None,
            evaluator_model: None,
            monthly_budget_usd: None,
            weekly_enabled: false,
            weekly_day: default_weekly_day(),
            weekly_hour: default_weekly_hour(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationRuleV1 {
    pub id: String,
    pub scope: String,
    pub app: String,
    pub title_pattern: Option<String>,
    pub category: String,
    pub activity_label: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ClassificationRuleV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_identifier(&self.id, "El id de la regla")?;
        if !matches!(self.scope.as_str(), "app" | "app-title" | "legacy") {
            return Err("El alcance de la regla no es válido".into());
        }
        validate_required_text(&self.app, 256, "La aplicación de la regla")?;
        match (self.scope.as_str(), self.title_pattern.as_deref()) {
            ("app", None) => {}
            ("legacy", None) => {}
            ("app-title", Some(pattern)) => {
                validate_required_text(pattern, 256, "El patrón de título")?;
            }
            ("app", Some(_)) | ("legacy", Some(_)) => {
                return Err("Una regla de aplicación no debe incluir título".into())
            }
            ("app-title", None) => {
                return Err("Una regla de aplicación y título requiere un patrón".into())
            }
            _ => unreachable!(),
        }
        validate_rule_category(&self.category)?;
        if let Some(label) = self.activity_label.as_deref() {
            validate_required_text(label, 200, "La etiqueta de actividad")?;
        }
        let created_at = parse_utc(&self.created_at)?;
        let updated_at = parse_utc(&self.updated_at)?;
        if updated_at < created_at {
            return Err("updatedAt no puede ser anterior a createdAt".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationOverrideV1 {
    pub block_id: String,
    pub event_start: String,
    pub event_end: String,
    pub category: String,
    pub activity_label: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionClassificationInputV1 {
    pub block_id: String,
    pub category: String,
    pub activity_label: Option<String>,
}

impl SessionClassificationInputV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.block_id.is_empty()
            || self.block_id.chars().count() > 512
            || self.block_id.chars().any(char::is_control)
        {
            return Err("La sesión no es válida".into());
        }
        validate_session_category(&self.category)?;
        if let Some(label) = self.activity_label.as_deref() {
            validate_required_text(label, 200, "La etiqueta de actividad")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleImpactV1 {
    pub sessions: usize,
    pub duration_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSuggestionV1 {
    pub rule: ClassificationRuleV1,
    pub impact: RuleImpactV1,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionClassificationResultV1 {
    pub block: TimelineBlockV1,
    pub suggestion: Option<RuleSuggestionV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewGroupV1 {
    pub id: String,
    pub app: String,
    pub title_pattern: Option<String>,
    pub sample_title: Option<String>,
    pub sessions: usize,
    pub duration_seconds: i64,
    pub first_seen: String,
    pub last_seen: String,
    pub browser_fallback: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReviewGroupInputV1 {
    pub from: String,
    pub to: String,
    pub app: String,
    pub title_pattern: Option<String>,
    pub category: String,
    pub activity_label: Option<String>,
    pub mode: String,
}

impl ApplyReviewGroupInputV1 {
    pub fn validate(&self) -> Result<(), String> {
        let from = parse_utc(&self.from)?;
        let to = parse_utc(&self.to)?;
        if to <= from || to - from > chrono::Duration::days(366) {
            return Err("El periodo de revisión no es válido".into());
        }
        validate_required_text(&self.app, 256, "La aplicación")?;
        if let Some(pattern) = self.title_pattern.as_deref() {
            validate_required_text(pattern, 256, "El patrón de título")?;
        }
        if !matches!(self.mode.as_str(), "sessions" | "rule") {
            return Err("La acción de revisión no es válida".into());
        }
        if self.mode == "rule" {
            validate_rule_category(&self.category)?;
        } else {
            validate_session_category(&self.category)?;
        }
        if let Some(label) = self.activity_label.as_deref() {
            validate_required_text(label, 200, "La etiqueta de actividad")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOperationV1 {
    pub id: String,
    pub mode: String,
    pub affected_sessions: usize,
    pub block_ids: Vec<String>,
    pub rule_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoReviewOperationInputV1 {
    pub block_ids: Vec<String>,
    pub rule_id: Option<String>,
}

pub fn validate_session_category(value: &str) -> Result<(), String> {
    if !is_category(value) || value == "unknown" {
        return Err("La categoría de clasificación no es válida".into());
    }
    Ok(())
}

pub fn validate_rule_category(value: &str) -> Result<(), String> {
    if !is_category(value) || matches!(value, "unknown" | "break") {
        return Err("La categoría de clasificación no es válida".into());
    }
    Ok(())
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
    #[serde(default)]
    pub deep_capture: DeepCaptureSettingsV2,
    #[serde(default)]
    pub ai: AiSettingsV2,
}

impl Default for SettingsV1 {
    fn default() -> Self {
        Self {
            schema_version: 2,
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
            deep_capture: DeepCaptureSettingsV2::default(),
            ai: AiSettingsV2::default(),
        }
    }
}

impl SettingsV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 2 {
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
        if !matches!(
            self.deep_capture.screenshot_interval_minutes,
            5 | 15 | 30 | 60
        ) {
            return Err("El intervalo de capturas debe ser 5, 15, 30 o 60 minutos".into());
        }
        if self.deep_capture.browser_text_allowlist.len() > 250
            || self.deep_capture.excluded_sites.len() > 250
        {
            return Err("La política contiene demasiados sitios".into());
        }
        for value in self
            .deep_capture
            .browser_text_allowlist
            .iter()
            .chain(self.deep_capture.excluded_sites.iter())
        {
            if value.trim().is_empty()
                || value.chars().count() > 253
                || value.chars().any(char::is_control)
            {
                return Err("La política de sitios contiene un dominio inválido".into());
            }
        }
        if self.deep_capture.excluded_apps.len() > 250
            || self
                .deep_capture
                .excluded_apps
                .iter()
                .any(|value| value.trim().is_empty() || value.chars().count() > 256)
        {
            return Err("La política de aplicaciones excluidas no es válida".into());
        }
        if self.ai.weekly_day > 6 || self.ai.weekly_hour > 23 {
            return Err("La agenda semanal de IA no es válida".into());
        }
        if self
            .ai
            .analyst_model
            .as_ref()
            .is_some_and(|value| !valid_model_slug(value))
            || self
                .ai
                .evaluator_model
                .as_ref()
                .is_some_and(|value| !valid_model_slug(value))
        {
            return Err("El identificador de modelo de OpenRouter no es válido".into());
        }
        if self.ai.enabled {
            let analyst = self
                .ai
                .analyst_model
                .as_deref()
                .ok_or("Selecciona un modelo analista")?;
            let evaluator = self
                .ai
                .evaluator_model
                .as_deref()
                .ok_or("Selecciona un modelo evaluador")?;
            if analyst == evaluator {
                return Err("El analista y el evaluador deben ser modelos distintos".into());
            }
            if !self
                .ai
                .monthly_budget_usd
                .is_some_and(|value| value.is_finite() && value > 0.0 && value <= 10_000.0)
            {
                return Err("Define un presupuesto mensual de IA mayor que cero".into());
            }
        } else if self
            .ai
            .monthly_budget_usd
            .is_some_and(|value| !value.is_finite() || value <= 0.0 || value > 10_000.0)
        {
            return Err("El presupuesto mensual de IA no es válido".into());
        }
        Ok(())
    }
}

fn valid_model_slug(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.chars().count() <= 200
        && value.contains('/')
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/' | ':')
        })
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
    AccessibilityPermissionRequired,
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
    pub processing_mode: String,
    pub last_cloud_run_at: Option<String>,
    pub capture_issue: Option<CaptureIssueCode>,
    pub native_action_error: Option<String>,
    pub experimental_intelligence: bool,
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
    pub activity_label: Option<String>,
    pub category: String,
    pub confidence: f64,
    pub afk: bool,
    pub source: String,
    pub check_in_id: Option<String>,
    pub classification_source: String,
    pub rule_id: Option<String>,
    pub correctable: bool,
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

pub fn canonicalize_candidate_label(label: &str) -> String {
    label
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_uppercase() {
                character.to_ascii_lowercase()
            } else {
                character
            }
        })
        .collect()
}

pub fn automation_candidate_id(label: &str) -> String {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET_BASIS;
    let canonical = canonicalize_candidate_label(label);
    for byte in canonical.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("candidate-{hash:016x}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowIntakeV1 {
    pub id: String,
    pub candidate_id: String,
    pub candidate_label: String,
    pub trigger: String,
    pub tools: Vec<String>,
    pub steps: Vec<String>,
    pub human_decisions: String,
    pub inputs: String,
    pub desired_output: String,
    pub frequency: String,
    pub minutes_per_run: u16,
    pub pain_points: String,
    pub created_at: String,
    pub updated_at: String,
}

impl WorkflowIntakeV1 {
    pub fn normalize_and_validate(mut self) -> Result<Self, String> {
        self.validate_raw_bounds()?;
        self.id = normalize_text(&self.id);
        self.candidate_id = normalize_text(&self.candidate_id);
        self.candidate_label = normalize_text(&self.candidate_label);
        self.trigger = normalize_text(&self.trigger);
        self.human_decisions = normalize_text(&self.human_decisions);
        self.inputs = normalize_text(&self.inputs);
        self.desired_output = normalize_text(&self.desired_output);
        self.frequency = normalize_text(&self.frequency);
        self.pain_points = normalize_text(&self.pain_points);
        self.created_at = parse_utc(&self.created_at)?.to_rfc3339();
        self.updated_at = parse_utc(&self.updated_at)?.to_rfc3339();
        self.tools = normalize_unique_list(self.tools);
        self.steps = self
            .steps
            .into_iter()
            .map(|value| normalize_text(&value))
            .collect();
        let expected_candidate_id = automation_candidate_id(&self.candidate_label);
        if self.candidate_id != expected_candidate_id {
            return Err(format!(
                "El id del candidato no corresponde a su nombre normalizado; se esperaba {expected_candidate_id}"
            ));
        }
        self.validate()?;
        Ok(self)
    }

    fn validate_raw_bounds(&self) -> Result<(), String> {
        for (value, max, label) in [
            (self.id.as_str(), 128, "El id del intake"),
            (self.candidate_id.as_str(), 128, "El id del candidato"),
            (
                self.candidate_label.as_str(),
                200,
                "El nombre del candidato",
            ),
            (self.trigger.as_str(), 500, "El disparador"),
            (
                self.human_decisions.as_str(),
                2_000,
                "Las decisiones humanas",
            ),
            (self.inputs.as_str(), 2_000, "Las entradas"),
            (self.desired_output.as_str(), 2_000, "El resultado deseado"),
            (self.frequency.as_str(), 200, "La frecuencia"),
            (self.pain_points.as_str(), 2_000, "Los puntos de dolor"),
            (self.created_at.as_str(), 64, "createdAt"),
            (self.updated_at.as_str(), 64, "updatedAt"),
        ] {
            if value.chars().count() > max {
                return Err(format!("{label} excede {max} caracteres"));
            }
        }
        if self.tools.len() > 25 || self.tools.iter().any(|tool| tool.chars().count() > 100) {
            return Err("Las herramientas exceden el tamaño permitido".into());
        }
        if self.steps.len() > 50 || self.steps.iter().any(|step| step.chars().count() > 500) {
            return Err("Los pasos exceden el tamaño permitido".into());
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_identifier(&self.id, "El id del intake")?;
        validate_identifier(&self.candidate_id, "El id del candidato")?;
        validate_required_text(&self.candidate_label, 200, "El nombre del candidato")?;
        validate_required_text(&self.trigger, 500, "El disparador")?;
        validate_required_text(&self.human_decisions, 2_000, "Las decisiones humanas")?;
        validate_required_text(&self.inputs, 2_000, "Las entradas")?;
        validate_required_text(&self.desired_output, 2_000, "El resultado deseado")?;
        validate_required_text(&self.frequency, 200, "La frecuencia")?;
        validate_required_text(&self.pain_points, 2_000, "Los puntos de dolor")?;
        if !(1..=1_440).contains(&self.minutes_per_run) {
            return Err("Los minutos por ejecución deben estar entre 1 y 1440".into());
        }
        if self.tools.is_empty() || self.tools.len() > 25 {
            return Err("Debe declarar entre 1 y 25 herramientas".into());
        }
        for tool in &self.tools {
            validate_required_text(tool, 100, "Una herramienta")?;
        }
        if self.steps.is_empty() || self.steps.len() > 50 {
            return Err("Debe declarar entre 1 y 50 pasos".into());
        }
        for step in &self.steps {
            validate_required_text(step, 500, "Un paso")?;
        }
        let created_at = parse_utc(&self.created_at)?;
        let updated_at = parse_utc(&self.updated_at)?;
        if updated_at < created_at {
            return Err("updatedAt no puede ser anterior a createdAt".into());
        }
        let latest_allowed = Utc::now() + chrono::Duration::minutes(5);
        if created_at > latest_allowed || updated_at > latest_allowed {
            return Err("Las fechas del intake no pueden estar en el futuro".into());
        }
        Ok(())
    }
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_unique_list(values: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let value = normalize_text(&value);
        if !normalized
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&value))
        {
            normalized.push(value);
        }
    }
    normalized
}

fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(format!(
            "{label} debe tener entre 1 y 128 caracteres ASCII seguros"
        ));
    }
    Ok(())
}

fn validate_required_text(value: &str, max: usize, label: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(format!(
            "{label} debe tener entre 1 y {max} caracteres sin controles"
        ));
    }
    Ok(())
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
    pub timeline: Vec<ExportTimelineBlockV2>,
    pub workflows: Vec<WorkflowIntakeV1>,
    pub automation_blueprints: Vec<crate::blueprint::AutomationBlueprintV1>,
    pub classification_rules: Vec<ClassificationRuleV1>,
    pub classification_overrides: Vec<ClassificationOverrideV1>,
    pub deletion_boundary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTimelineBlockV2 {
    pub id: String,
    pub start: String,
    pub end: String,
    pub duration_seconds: i64,
    pub app: String,
    pub activity_label: Option<String>,
    pub category: String,
    pub confidence: f64,
    pub afk: bool,
    pub source: String,
    pub check_in_id: Option<String>,
    pub classification_source: String,
    pub rule_id: Option<String>,
}

impl From<&TimelineBlockV1> for ExportTimelineBlockV2 {
    fn from(value: &TimelineBlockV1) -> Self {
        Self {
            id: value.id.clone(),
            start: value.start.clone(),
            end: value.end.clone(),
            duration_seconds: value.duration_seconds,
            app: value.app.clone(),
            activity_label: value.activity_label.clone(),
            category: value.category.clone(),
            confidence: value.confidence,
            afk: value.afk,
            source: value.source.clone(),
            check_in_id: value.check_in_id.clone(),
            classification_source: value.classification_source.clone(),
            rule_id: value.rule_id.clone(),
        }
    }
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

    fn workflow() -> WorkflowIntakeV1 {
        let candidate_label = "  Preparar   reporte semanal ";
        WorkflowIntakeV1 {
            id: " workflow-1 ".into(),
            candidate_id: automation_candidate_id(&normalize_text(candidate_label)),
            candidate_label: candidate_label.into(),
            trigger: " Cada   viernes\npor la tarde ".into(),
            tools: vec![" Sheets ".into(), "sheets".into(), " Slack ".into()],
            steps: vec![" Extraer  datos ".into(), " Revisar totales ".into()],
            human_decisions: " Aprobar excepciones ".into(),
            inputs: " Ventas y costos ".into(),
            desired_output: " Resumen validado ".into(),
            frequency: " Semanal ".into(),
            minutes_per_run: 45,
            pain_points: " Copiar y pegar ".into(),
            created_at: "2025-01-01T10:00:00-05:00".into(),
            updated_at: "2025-01-02T10:00:00-05:00".into(),
        }
    }

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
            (
                CaptureIssueCode::AccessibilityPermissionRequired,
                "accessibility-permission-required",
            ),
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

    #[test]
    fn candidate_ids_use_stable_fnv1a_over_trimmed_utf8() {
        assert_eq!(
            automation_candidate_id("hello"),
            "candidate-a430d84680aabd0b"
        );
        assert_eq!(
            automation_candidate_id("  HELLO  "),
            "candidate-a430d84680aabd0b"
        );
        assert_eq!(
            automation_candidate_id("  hello  "),
            automation_candidate_id("hello")
        );
        assert_eq!(
            automation_candidate_id("Preparar reporte"),
            automation_candidate_id("preparar reporte")
        );
        assert_eq!(
            canonicalize_candidate_label("  PREPARAR 🚀 Á  "),
            "preparar 🚀 Á"
        );
        assert_ne!(
            automation_candidate_id("Área 🚀"),
            automation_candidate_id("área 🚀")
        );
        let stable_unicode = automation_candidate_id("Conciliación 💸");
        for _ in 0..32 {
            assert_eq!(automation_candidate_id("Conciliación 💸"), stable_unicode);
        }
        assert_ne!(
            automation_candidate_id("Conciliación 💸"),
            automation_candidate_id("Conciliacion 💸")
        );
        assert_ne!(
            automation_candidate_id("Enviar 🚀"),
            automation_candidate_id("Enviar 🛠️")
        );
        let id = automation_candidate_id("日本語💡");
        assert_eq!(id.len(), "candidate-".len() + 16);
        assert!(id["candidate-".len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')));
        let mut intake = workflow();
        intake.candidate_label = "日本語💡".into();
        intake.candidate_id = id;
        assert!(intake.normalize_and_validate().is_ok());
    }

    #[test]
    fn workflow_intake_rejects_candidate_id_not_derived_from_normalized_label() {
        let mut intake = workflow();
        intake.candidate_id = automation_candidate_id("Otro candidato");
        let error = intake.normalize_and_validate().unwrap_err();
        assert!(error.contains("no corresponde a su nombre normalizado"));
    }

    #[test]
    fn workflow_intake_normalizes_and_uses_camel_case_wire_keys() {
        let intake = workflow().normalize_and_validate().unwrap();
        assert_eq!(intake.id, "workflow-1");
        assert_eq!(intake.candidate_label, "Preparar reporte semanal");
        assert_eq!(intake.trigger, "Cada viernes por la tarde");
        assert_eq!(intake.tools, ["Sheets", "Slack"]);
        assert_eq!(intake.created_at, "2025-01-01T15:00:00+00:00");
        let json = serde_json::to_value(&intake).unwrap();
        assert_eq!(
            json["candidateId"],
            automation_candidate_id("Preparar reporte semanal")
        );
        assert_eq!(json["minutesPerRun"], 45);
        assert!(json.get("candidate_id").is_none());
    }

    #[test]
    fn workflow_intake_rejects_unsafe_or_unbounded_values() {
        let mut invalid_id = workflow();
        invalid_id.id = "workflow/../../official".into();
        assert!(invalid_id.normalize_and_validate().is_err());

        let mut invalid_minutes = workflow();
        invalid_minutes.minutes_per_run = 0;
        assert!(invalid_minutes.normalize_and_validate().is_err());

        let mut empty_steps = workflow();
        empty_steps.steps = vec![" \n ".into()];
        assert!(empty_steps.normalize_and_validate().is_err());

        let mut reversed_dates = workflow();
        reversed_dates.updated_at = "2024-12-31T10:00:00Z".into();
        assert!(reversed_dates.normalize_and_validate().is_err());

        let mut oversized = workflow();
        oversized.pain_points = "x".repeat(2_001);
        assert!(oversized.normalize_and_validate().is_err());

        let mut oversized_before_normalization = workflow();
        oversized_before_normalization.tools = vec!["Sheets".into(); 26];
        assert!(oversized_before_normalization
            .normalize_and_validate()
            .is_err());
    }
}
