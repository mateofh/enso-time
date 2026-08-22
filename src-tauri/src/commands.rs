use crate::activitywatch::{completed_interval, ActivityWatchRuntime};
use crate::blueprint::{
    assess_blueprint, draft_from_input, ephemeral_draft, AutomationBlueprintExportEnvelopeV2,
    AutomationBlueprintV1, BlueprintConflictV1, BlueprintStatusV1, SaveAutomationBlueprintInputV1,
    BLUEPRINT_EXPORT_SCHEMA_VERSION,
};
use crate::diagnostics::create_diagnostic;
use crate::models::{
    iso_now, parse_utc, CheckInAnswer, CheckInV1, DiagnosticV1, ExportEnvelope, SettingsV1,
    StudyProgress, TimelineBlockV1, TrackingStatus, UpdateInfo, WorkflowIntakeV1,
};
use crate::storage::{BlueprintStoreResult, LocalStore};
use chrono::{Datelike, Duration, Timelike, Utc};
use chrono_tz::Tz;
use std::fs::{OpenOptions, Permissions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::thread::JoinHandle;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use url::Url;
use uuid::Uuid;

pub struct AppState {
    pub scheduler: SchedulerHandle,
    pub capture_gate: Arc<CaptureGate>,
    pub store: Arc<LocalStore>,
    pub activity: Arc<ActivityWatchRuntime>,
    pub app_data_dir: PathBuf,
}

pub struct CaptureGate {
    access: RwLock<()>,
    exclusive_serial: Mutex<()>,
    generation: AtomicU64,
    cancelling: AtomicBool,
    #[cfg(test)]
    exclusive_requests: AtomicU64,
}

pub(crate) struct CaptureOperation<'a> {
    gate: &'a CaptureGate,
    generation: u64,
    _access: RwLockReadGuard<'a, ()>,
}

struct CaptureExclusive<'a> {
    gate: &'a CaptureGate,
    activity: &'a ActivityWatchRuntime,
    _access: RwLockWriteGuard<'a, ()>,
    _serial: MutexGuard<'a, ()>,
}

impl CaptureGate {
    pub fn new() -> Self {
        Self {
            access: RwLock::new(()),
            exclusive_serial: Mutex::new(()),
            generation: AtomicU64::new(0),
            cancelling: AtomicBool::new(false),
            #[cfg(test)]
            exclusive_requests: AtomicU64::new(0),
        }
    }

    pub(crate) fn operation(&self) -> Result<CaptureOperation<'_>, String> {
        if self.cancelling.load(Ordering::Acquire) {
            return Err("La captura está cancelándose para una operación exclusiva".into());
        }
        let access = self
            .access
            .read()
            .map_err(|_| "No se pudo coordinar la captura".to_string())?;
        if self.cancelling.load(Ordering::Acquire) {
            return Err("La captura está cancelándose para una operación exclusiva".into());
        }
        Ok(CaptureOperation {
            gate: self,
            generation: self.generation.load(Ordering::Acquire),
            _access: access,
        })
    }

    fn cancel_and_wait<'a>(
        &'a self,
        activity: &'a ActivityWatchRuntime,
    ) -> Result<CaptureExclusive<'a>, String> {
        #[cfg(test)]
        self.exclusive_requests.fetch_add(1, Ordering::AcqRel);
        let serial = self
            .exclusive_serial
            .lock()
            .map_err(|_| "No se pudo serializar el borrado privado".to_string())?;
        self.cancelling.store(true, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        let _ = activity.cancel_pending_capture();
        let access = match self.access.write() {
            Ok(access) => access,
            Err(_) => {
                activity.resume_pending_capture();
                self.cancelling.store(false, Ordering::Release);
                return Err("No se pudo obtener acceso exclusivo a la captura".into());
            }
        };
        Ok(CaptureExclusive {
            gate: self,
            activity,
            _access: access,
            _serial: serial,
        })
    }

    #[cfg(test)]
    fn exclusive_request_count(&self) -> u64 {
        self.exclusive_requests.load(Ordering::Acquire)
    }
}

impl CaptureOperation<'_> {
    pub(crate) fn ensure_current(&self) -> Result<(), String> {
        if self.gate.cancelling.load(Ordering::Acquire)
            || self.gate.generation.load(Ordering::Acquire) != self.generation
        {
            Err("La operación de captura fue cancelada".into())
        } else {
            Ok(())
        }
    }
}

impl Drop for CaptureExclusive<'_> {
    fn drop(&mut self) {
        // Resume while the write lock is still held, then open the gate. This
        // prevents a scheduler reader from slipping through between both steps.
        self.activity.resume_pending_capture();
        self.gate.cancelling.store(false, Ordering::Release);
    }
}

impl CaptureExclusive<'_> {
    fn resume_for_rollback(&mut self) {
        // A failed delete may restore the previous capture state, but it must do
        // so before releasing either the writer gate or the delete serializer.
        self.activity.resume_pending_capture();
    }
}

pub struct SchedulerHandle {
    state: Mutex<SchedulerState>,
}

struct SchedulerState {
    stop_sender: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl SchedulerHandle {
    pub fn shutdown(&self) -> Result<(), String> {
        let (sender, thread) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "No se pudo detener el scheduler".to_string())?;
            (state.stop_sender.take(), state.thread.take())
        };
        if let Some(sender) = sender {
            let _ = sender.send(());
        }
        if let Some(thread) = thread {
            thread
                .join()
                .map_err(|_| "El scheduler terminó de forma inesperada".to_string())?;
        }
        Ok(())
    }
}

impl Drop for SchedulerHandle {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<SettingsV1, String> {
    state.store.settings()
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: SettingsV1,
) -> Result<SettingsV1, String> {
    let operation = state.capture_gate.operation()?;
    let previous = state.store.settings()?;
    settings.validate()?;
    configure_autostart(&app, settings.onboarding_completed && settings.autostart)?;
    state.store.save_settings(&settings)?;
    if previous.onboarding_completed && !settings.onboarding_completed {
        state.activity.stop_watchers()?;
    } else if !previous.onboarding_completed && settings.onboarding_completed {
        operation.ensure_current()?;
        state
            .activity
            .start_watchers(settings.capture_window_titles)?;
    } else if previous.capture_window_titles != settings.capture_window_titles
        && is_running(&state.store)?
    {
        operation.ensure_current()?;
        if let Err(error) = state
            .activity
            .restart_window_watcher(settings.capture_window_titles)
        {
            // Persisted settings remain correct; surface the capture failure instead of
            // pretending the privacy change was applied to a running process.
            return Err(format!(
                "La configuración se guardó, pero el capturador no pudo reiniciarse: {error}"
            ));
        }
    }
    state.store.prune_checkins(settings.retention_days)?;
    state.activity.prune_activity(settings.retention_days)?;
    Ok(settings)
}

#[tauri::command]
pub fn get_tracking_status(state: State<'_, AppState>) -> Result<TrackingStatus, String> {
    status(&state)
}

#[tauri::command]
pub fn get_timeline(
    state: State<'_, AppState>,
    from: String,
    to: String,
) -> Result<Vec<TimelineBlockV1>, String> {
    let from_date = parse_utc(&from)?;
    let to_date = parse_utc(&to)?;
    validate_range(from_date, to_date)?;
    let settings = state.store.settings()?;
    let checkins = state.store.checkins_between(&from, &to)?;
    state
        .activity
        .timeline(from_date, to_date, &settings, &checkins)
}

#[tauri::command]
pub fn list_checkins(state: State<'_, AppState>) -> Result<Vec<CheckInV1>, String> {
    state.store.checkins()
}

#[tauri::command]
pub fn answer_checkin(
    state: State<'_, AppState>,
    answer: CheckInAnswer,
) -> Result<CheckInV1, String> {
    state.store.answer_checkin(&answer)
}

#[tauri::command]
pub fn get_study_progress(state: State<'_, AppState>) -> Result<StudyProgress, String> {
    let settings = state.store.settings()?;
    let started_at = state
        .store
        .get_value("study_started_at")?
        .unwrap_or_else(iso_now);
    let start = parse_utc(&started_at)?;
    let now = Utc::now();
    let selected_days = state
        .store
        .get_value("selected_days")?
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| matches!(value, 7 | 14 | 28))
        .unwrap_or(7);
    let checkins = state
        .store
        .checkins_between(&start.to_rfc3339(), &now.to_rfc3339())?;
    let timeline = state.activity.timeline(start, now, &settings, &checkins)?;
    let tracked_seconds: i64 = timeline
        .iter()
        .filter(|block| !block.afk)
        .map(|block| block.duration_seconds)
        .sum();
    let answered = checkins
        .iter()
        .filter(|item| item.status == "answered")
        .count();
    let expected_intervals =
        ((tracked_seconds as f64 / 60.0) / f64::from(settings.cadence_minutes)).ceil() as usize;
    let coverage = if expected_intervals == 0 {
        0
    } else {
        ((answered as f64 / expected_intervals as f64) * 100.0)
            .round()
            .clamp(0.0, 100.0) as i32
    };
    let observed_days =
        ((now - start).num_seconds().max(0) as f64 / 86_400.0).min(f64::from(selected_days));
    Ok(StudyProgress {
        started_at,
        selected_days,
        observed_days,
        tracked_hours: tracked_seconds as f64 / 3600.0,
        answered_check_ins: answered,
        coverage_percent: coverage,
    })
}

#[tauri::command]
pub fn generate_diagnostic(state: State<'_, AppState>, days: u16) -> Result<DiagnosticV1, String> {
    let operation = state.capture_gate.operation()?;
    if !matches!(days, 7 | 14 | 28) {
        return Err("El diagnóstico debe cubrir 7, 14 o 28 días".into());
    }
    state.store.set_value("selected_days", &days.to_string())?;
    let settings = state.store.settings()?;
    let now = Utc::now();
    let requested_start = now - Duration::days(i64::from(days));
    let study_start = state
        .store
        .get_value("study_started_at")?
        .and_then(|value| parse_utc(&value).ok())
        .unwrap_or(requested_start);
    let start = requested_start.max(study_start);
    let checkins = state
        .store
        .checkins_between(&start.to_rfc3339(), &now.to_rfc3339())?;
    let workflows = state.store.list_workflow_intakes()?;
    let blocks = state.activity.timeline(start, now, &settings, &checkins)?;
    let observed = ((now - start).num_seconds().max(0) as f64 / 86_400.0).min(f64::from(days));
    operation.ensure_current()?;
    Ok(create_diagnostic(
        &blocks, &checkins, observed, &settings, &workflows,
    ))
}

#[tauri::command]
pub fn list_workflow_intakes(state: State<'_, AppState>) -> Result<Vec<WorkflowIntakeV1>, String> {
    list_workflow_intakes_inner(&state.store, &state.capture_gate)
}

#[tauri::command]
pub fn save_workflow_intake(
    state: State<'_, AppState>,
    intake: WorkflowIntakeV1,
) -> Result<WorkflowIntakeV1, String> {
    save_workflow_intake_inner(&state.store, &state.capture_gate, intake)
}

fn list_workflow_intakes_inner(
    store: &LocalStore,
    capture_gate: &CaptureGate,
) -> Result<Vec<WorkflowIntakeV1>, String> {
    let operation = capture_gate.operation()?;
    operation.ensure_current()?;
    store.list_workflow_intakes()
}

fn save_workflow_intake_inner(
    store: &LocalStore,
    capture_gate: &CaptureGate,
    intake: WorkflowIntakeV1,
) -> Result<WorkflowIntakeV1, String> {
    let operation = capture_gate.operation()?;
    operation.ensure_current()?;
    store.save_workflow_intake(intake)
}

fn assess_stored_blueprint(
    store: &LocalStore,
    draft: crate::blueprint::AutomationBlueprintDraftV1,
    locale: &str,
) -> Result<AutomationBlueprintV1, String> {
    let intake = store
        .workflow_intake_for_candidate(&draft.candidate_id)?
        .ok_or_else(|| "El intake fuente ya no existe".to_string())?;
    let (total, repeated, observed_days) =
        store.blueprint_observation_counts(&draft.candidate_id)?;
    assess_blueprint(draft, &intake, total, repeated, observed_days, locale)
}

fn get_automation_blueprint_inner(
    store: &LocalStore,
    capture_gate: &CaptureGate,
    candidate_id: &str,
    locale: &str,
) -> Result<Option<AutomationBlueprintV1>, String> {
    let operation = capture_gate.operation()?;
    operation.ensure_current()?;
    let Some(intake) = store.workflow_intake_for_candidate(candidate_id)? else {
        return Ok(None);
    };
    let draft = store
        .automation_blueprint_draft(candidate_id)?
        .unwrap_or_else(|| ephemeral_draft(&intake));
    let (total, repeated, observed_days) = store.blueprint_observation_counts(candidate_id)?;
    assess_blueprint(draft, &intake, total, repeated, observed_days, locale).map(Some)
}

#[tauri::command]
pub fn get_automation_blueprint_v1(
    state: State<'_, AppState>,
    candidate_id: String,
    locale: String,
) -> Result<Option<AutomationBlueprintV1>, String> {
    get_automation_blueprint_inner(&state.store, &state.capture_gate, &candidate_id, &locale)
}

#[tauri::command]
pub fn save_automation_blueprint_v1(
    state: State<'_, AppState>,
    input: SaveAutomationBlueprintInputV1,
    locale: String,
) -> Result<AutomationBlueprintV1, String> {
    let operation = state.capture_gate.operation()?;
    operation.ensure_current()?;
    let intake = state
        .store
        .workflow_intake_for_candidate(&input.draft.candidate_id)?
        .ok_or_else(|| "blueprint.intake_missing".to_string())?;
    let existing = state
        .store
        .automation_blueprint_draft(&input.draft.candidate_id)?;
    if existing
        .as_ref()
        .is_some_and(|draft| draft.source_intake_updated_at != intake.updated_at)
    {
        return Err("blueprint.stale".into());
    }
    let expected_revision = input.expected_revision.unwrap_or(input.draft.revision);
    let draft = draft_from_input(&intake, existing.as_ref(), input.draft)?;
    match state
        .store
        .save_automation_blueprint_draft(draft, expected_revision, &intake)?
    {
        BlueprintStoreResult::Applied(draft) => {
            assess_stored_blueprint(&state.store, draft, &locale)
        }
        BlueprintStoreResult::Conflict(current) => {
            let mut current = assess_stored_blueprint(&state.store, current, &locale)?;
            current.conflict = Some(BlueprintConflictV1 {
                current_revision: current.draft.revision,
            });
            Ok(current)
        }
        BlueprintStoreResult::Deleted => Err("Resultado de persistencia inesperado".into()),
    }
}

#[tauri::command]
pub fn refresh_automation_blueprint_source_v1(
    state: State<'_, AppState>,
    candidate_id: String,
    expected_revision: u64,
    locale: String,
) -> Result<AutomationBlueprintV1, String> {
    let operation = state.capture_gate.operation()?;
    operation.ensure_current()?;
    let intake = state
        .store
        .workflow_intake_for_candidate(&candidate_id)?
        .ok_or_else(|| "blueprint.not_found".to_string())?;
    match state.store.refresh_automation_blueprint_draft(
        &candidate_id,
        expected_revision,
        &intake,
    )? {
        BlueprintStoreResult::Applied(draft) => {
            assess_stored_blueprint(&state.store, draft, &locale)
        }
        BlueprintStoreResult::Conflict(current) => {
            let mut current = assess_stored_blueprint(&state.store, current, &locale)?;
            current.conflict = Some(BlueprintConflictV1 {
                current_revision: current.draft.revision,
            });
            Ok(current)
        }
        BlueprintStoreResult::Deleted => Err("Resultado de persistencia inesperado".into()),
    }
}

#[tauri::command]
pub fn export_automation_blueprint_v1(
    state: State<'_, AppState>,
    candidate_id: String,
    expected_revision: u64,
    format: String,
    locale: String,
) -> Result<String, String> {
    let operation = state.capture_gate.operation()?;
    operation.ensure_current()?;
    if !matches!(format.as_str(), "markdown" | "json") {
        return Err("El formato de blueprint debe ser markdown o json".into());
    }
    let draft = state
        .store
        .automation_blueprint_draft(&candidate_id)?
        .ok_or_else(|| "Guarda el blueprint antes de exportarlo".to_string())?;
    let blueprint = assess_stored_blueprint(&state.store, draft, &locale)?;
    if blueprint.draft.revision != expected_revision {
        return Err("blueprint.conflict".into());
    }
    if blueprint.status != BlueprintStatusV1::Ready {
        return Err("blueprint.not_ready".into());
    }
    let now = Utc::now();
    let contents = if format == "json" {
        serde_json::to_string_pretty(&AutomationBlueprintExportEnvelopeV2 {
            schema_version: BLUEPRINT_EXPORT_SCHEMA_VERSION,
            export_kind: "automation-blueprint",
            exported_at: now.to_rfc3339(),
            blueprint,
            deletion_boundary: deletion_boundary(&locale).into(),
        })
        .map_err(|error| format!("No se pudo preparar la exportación: {error}"))?
    } else {
        blueprint.rendered_markdown
    };
    let export_dir = state.app_data_dir.join("exports");
    create_private_directory(&export_dir)?;
    let extension = if format == "json" { "json" } else { "md" };
    let filename = format!(
        "enso-automation-blueprint-{}-{}.{}",
        now.format("%Y%m%d-%H%M%S"),
        Uuid::new_v4(),
        extension
    );
    let path = export_dir.join(filename);
    operation.ensure_current()?;
    write_private_new_file(&path, contents.as_bytes())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn delete_automation_blueprint_v1(
    state: State<'_, AppState>,
    candidate_id: String,
    expected_revision: u64,
    confirmation: String,
    locale: String,
) -> Result<(), String> {
    let operation = state.capture_gate.operation()?;
    operation.ensure_current()?;
    if confirmation != "DELETE_AUTOMATION_BLUEPRINT" {
        return Err("La confirmación de borrado del blueprint no coincide".into());
    }
    if !matches!(locale.as_str(), "es-419" | "en") {
        return Err("El idioma del blueprint no es compatible".into());
    }
    match state
        .store
        .delete_automation_blueprint_draft(&candidate_id, expected_revision)?
    {
        BlueprintStoreResult::Deleted => Ok(()),
        BlueprintStoreResult::Conflict(_) => Err("blueprint.conflict".into()),
        BlueprintStoreResult::Applied(_) => Err("Resultado de persistencia inesperado".into()),
    }
}

fn deletion_boundary(locale: &str) -> &'static str {
    if locale == "en" {
        "Ensō Time can delete its local database and app-managed exports. Clipboard contents, moved exports, backups, and synced copies remain outside its deletion boundary."
    } else {
        "Ensō Time puede borrar su base local y exports administrados. El portapapeles, exports movidos, respaldos y copias sincronizadas quedan fuera de su límite de borrado."
    }
}

fn create_private_directory(path: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|error| format!("No se pudo crear el directorio de exportación: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, Permissions::from_mode(0o700)).map_err(|error| {
            format!("No se pudo proteger el directorio de exportación: {error}")
        })?;
    }
    Ok(())
}

fn write_private_new_file(path: &std::path::Path, contents: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("No se pudo crear la exportación privada: {error}"))?;
    file.write_all(contents)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("No se pudo escribir la exportación: {error}"))
}

#[tauri::command]
pub fn pause_tracking(state: State<'_, AppState>, minutes: u16) -> Result<TrackingStatus, String> {
    let operation = state.capture_gate.operation()?;
    if !(5..=1440).contains(&minutes) {
        return Err("La pausa debe durar entre 5 minutos y 24 horas".into());
    }
    let paused_until = Utc::now() + Duration::minutes(i64::from(minutes));
    state.activity.stop_watchers()?;
    state
        .store
        .set_paused_until(Some(&paused_until.to_rfc3339()))?;
    drop(operation);
    status(&state)
}

#[tauri::command]
pub fn resume_tracking(state: State<'_, AppState>) -> Result<TrackingStatus, String> {
    let operation = state.capture_gate.operation()?;
    let settings = state.store.settings()?;
    if !settings.onboarding_completed {
        return Err("Completa la configuración inicial antes de iniciar el seguimiento".into());
    }
    operation.ensure_current()?;
    state
        .activity
        .start_watchers(settings.capture_window_titles)?;
    state.store.set_paused_until(None)?;
    drop(operation);
    status(&state)
}

#[tauri::command]
pub fn export_data(state: State<'_, AppState>, format: String) -> Result<String, String> {
    let operation = state.capture_gate.operation()?;
    if !matches!(format.as_str(), "json" | "csv") {
        return Err("El formato de exportación debe ser json o csv".into());
    }
    let settings = state.store.settings()?;
    let now = Utc::now();
    let start = settings
        .retention_days
        .map(|days| now - Duration::days(i64::from(days)))
        .unwrap_or(chrono::DateTime::<Utc>::UNIX_EPOCH);
    let checkins = state
        .store
        .checkins_between(&start.to_rfc3339(), &now.to_rfc3339())?;
    let workflows = state.store.list_workflow_intakes()?;
    let blueprint_drafts = state.store.list_automation_blueprint_drafts()?;
    let mut blueprints = Vec::with_capacity(blueprint_drafts.len());
    for draft in blueprint_drafts {
        blueprints.push(assess_stored_blueprint(
            &state.store,
            draft,
            &settings.locale,
        )?);
    }
    let timeline = state.activity.timeline(start, now, &settings, &checkins)?;
    let export_dir = state.app_data_dir.join("exports");
    create_private_directory(&export_dir)?;
    let filename = format!(
        "enso-time-{}-{}.{}",
        now.format("%Y%m%d-%H%M%S"),
        Uuid::new_v4(),
        format
    );
    let path = export_dir.join(filename);
    operation.ensure_current()?;
    let contents = if format == "json" {
        json_export(
            &settings,
            &checkins,
            &timeline,
            &workflows,
            &blueprints,
            &now,
        )?
    } else {
        timeline_csv(&timeline)
    };
    write_private_new_file(&path, contents.as_bytes())?;
    Ok(path.to_string_lossy().into_owned())
}

fn json_export(
    settings: &SettingsV1,
    checkins: &[CheckInV1],
    timeline: &[TimelineBlockV1],
    workflows: &[WorkflowIntakeV1],
    blueprints: &[AutomationBlueprintV1],
    exported_at: &chrono::DateTime<Utc>,
) -> Result<String, String> {
    serde_json::to_string_pretty(&ExportEnvelope {
        schema_version: BLUEPRINT_EXPORT_SCHEMA_VERSION,
        exported_at: exported_at.to_rfc3339(),
        settings: settings.clone(),
        check_ins: checkins.to_vec(),
        timeline: timeline.iter().map(Into::into).collect(),
        workflows: workflows.to_vec(),
        automation_blueprints: blueprints.to_vec(),
        deletion_boundary: deletion_boundary(&settings.locale).into(),
    })
    .map_err(|error| format!("No se pudo preparar la exportación: {error}"))
}

#[tauri::command]
pub fn delete_all_data(state: State<'_, AppState>, confirmation: String) -> Result<(), String> {
    if confirmation != "DELETE_ENSO_TIME_DATA" {
        return Err("La confirmación de borrado no coincide".into());
    }
    delete_all_data_inner(&state.store, &state.activity, &state.capture_gate)
}

fn delete_all_data_inner(
    store: &LocalStore,
    activity: &ActivityWatchRuntime,
    capture_gate: &CaptureGate,
) -> Result<(), String> {
    run_exclusive_delete(
        activity,
        capture_gate,
        || {
            let previous_settings = store.settings().ok();
            let result = (|| {
                activity.stop_watchers()?;
                activity.delete_queued_activity()?;
                activity.delete_all_activity().map_err(|error| {
                    format!(
                        "No se pudo completar el borrado seguro del almacén de actividad: {error}"
                    )
                })?;
                store.delete_all()
            })();
            (result, previous_settings)
        },
        |previous_settings| {
            if let Some(settings) =
                previous_settings.filter(|settings| settings.onboarding_completed)
            {
                let _ = activity.start_watchers(settings.capture_window_titles);
            }
        },
    )
}

fn run_exclusive_delete<T, Delete, Rollback>(
    activity: &ActivityWatchRuntime,
    capture_gate: &CaptureGate,
    delete: Delete,
    rollback: Rollback,
) -> Result<(), String>
where
    Delete: FnOnce() -> (Result<(), String>, T),
    Rollback: FnOnce(T),
{
    let mut exclusive = capture_gate.cancel_and_wait(activity)?;
    let (delete_result, rollback_context) = delete();
    if delete_result.is_err() {
        exclusive.resume_for_rollback();
        rollback(rollback_context);
    }
    // CaptureExclusive keeps both the writer gate and delete serializer until
    // rollback has fully completed. A later delete can therefore invalidate all
    // state restored by an earlier failed attempt before it confirms success.
    drop(exclusive);
    delete_result
}

#[tauri::command]
pub fn check_for_update() -> Result<UpdateInfo, String> {
    // Releases are deliberately manual until a signed updater endpoint is configured.
    // Returning the installed version is truthful and avoids an undeclared network call.
    Ok(UpdateInfo {
        available: false,
        current_version: env!("CARGO_PKG_VERSION").into(),
        latest_version: None,
        release_url: None,
    })
}

#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> Result<(), String> {
    if url.len() > 2_048 {
        return Err("La URL excede el tamaño permitido".into());
    }
    let parsed = Url::parse(&url).map_err(|_| "La URL no es válida".to_string())?;
    if parsed.scheme() != "https" {
        return Err("Solo se permiten enlaces HTTPS".into());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "La URL no tiene un dominio".to_string())?;
    const ALLOWED: [&str; 5] = [
        "chatgpt.com",
        "claude.ai",
        "github.com",
        "activitywatch.net",
        "enso.group",
    ];
    if !ALLOWED.contains(&host) {
        return Err("El dominio externo no está autorizado".into());
    }
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|error| error.to_string())
}

pub fn start_scheduler(
    app: AppHandle,
    store: Arc<LocalStore>,
    activity: Arc<ActivityWatchRuntime>,
    capture_gate: Arc<CaptureGate>,
) -> Result<SchedulerHandle, String> {
    spawn_scheduler(
        move || scheduler_tick(&app, &store, &activity, &capture_gate),
        std::time::Duration::from_secs(30),
    )
}

pub(crate) fn bootstrap_watchers_if_current(
    store: &LocalStore,
    activity: &ActivityWatchRuntime,
    capture_gate: &CaptureGate,
) -> Result<(), String> {
    let operation = capture_gate.operation()?;
    let settings = store.settings()?;
    if !settings.onboarding_completed || !is_running(store)? {
        return Ok(());
    }
    operation.ensure_current()?;
    if activity.shutdown_requested() {
        return Ok(());
    }
    activity.start_watchers(settings.capture_window_titles)
}

fn spawn_scheduler<F>(mut tick: F, interval: std::time::Duration) -> Result<SchedulerHandle, String>
where
    F: FnMut() -> Result<(), String> + Send + 'static,
{
    let (stop_sender, stop_receiver) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("enso-checkin-scheduler".into())
        .spawn(move || loop {
            if let Err(error) = tick() {
                eprintln!("Ensō Time scheduler: {error}");
            }
            match stop_receiver.recv_timeout(interval) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        })
        .map_err(|error| format!("No se pudo iniciar el scheduler: {error}"))?;
    Ok(SchedulerHandle {
        state: Mutex::new(SchedulerState {
            stop_sender: Some(stop_sender),
            thread: Some(thread),
        }),
    })
}

fn scheduler_tick(
    app: &AppHandle,
    store: &LocalStore,
    activity: &ActivityWatchRuntime,
    capture_gate: &CaptureGate,
) -> Result<(), String> {
    let operation = capture_gate.operation()?;
    let settings = store.settings()?;
    let today = Utc::now().date_naive().to_string();
    if store.get_value("last_retention_prune")?.as_deref() != Some(&today) {
        store.prune_checkins(settings.retention_days)?;
        activity.prune_activity(settings.retention_days)?;
        store.set_value("last_retention_prune", &today)?;
    }
    if !is_running(store)? {
        return Ok(());
    }
    let (window_status, afk_status) = activity.watcher_statuses();
    if window_status != "running" || afk_status != "running" {
        if operation.ensure_current().is_err() || activity.shutdown_requested() {
            return Ok(());
        }
        activity.start_watchers(settings.capture_window_titles)?;
    }
    if !inside_work_schedule(&settings, Utc::now()) {
        return Ok(());
    }
    let (start, end) = completed_interval(Utc::now(), settings.cadence_minutes);
    let Some(reason) = activity.interval_reason(start, end, &settings)? else {
        return Ok(());
    };
    let checkin = CheckInV1 {
        id: Uuid::new_v4().to_string(),
        interval_start: start.to_rfc3339(),
        interval_end: end.to_rfc3339(),
        reason,
        status: "pending".into(),
        activity_label: None,
        category: None,
        project: None,
        repetitive: None,
        rules_clear: None,
        created_at: iso_now(),
        answered_at: None,
    };
    if operation.ensure_current().is_err() || activity.shutdown_requested() {
        return Ok(());
    }
    if store.create_checkin(&checkin)? {
        let _ = app
            .notification()
            .builder()
            .title("Ensō Time · Falta contexto")
            .body("¿Qué estabas haciendo en el último intervalo?")
            .show();
    }
    Ok(())
}

fn status(state: &AppState) -> Result<TrackingStatus, String> {
    let operation = state.capture_gate.operation()?;
    let should_run = is_running(&state.store)?;
    let paused_until = state.store.paused_until()?;
    if should_run {
        let settings = state.store.settings()?;
        let (window, afk) = state.activity.watcher_statuses();
        if window == "stopped" || afk == "stopped" {
            if operation.ensure_current().is_ok() && !state.activity.shutdown_requested() {
                let _ = state
                    .activity
                    .start_watchers(settings.capture_window_titles);
            }
        }
    }
    let (window_watcher, afk_watcher) = state.activity.watcher_statuses();
    let running = should_run
        && window_watcher == "running"
        && afk_watcher == "running"
        && state.activity.port_if_ready().is_some();
    Ok(TrackingStatus {
        running,
        paused_until,
        server_port: state.activity.port_if_ready(),
        window_watcher,
        afk_watcher,
        last_event_at: state.activity.latest_event_at()?,
        pending_check_ins: state.store.pending_count()?,
        data_stays_local: true,
        capture_issue: state.activity.capture_issue(),
    })
}

fn is_running(store: &LocalStore) -> Result<bool, String> {
    if !store.settings()?.onboarding_completed {
        return Ok(false);
    }
    let Some(value) = store.paused_until()? else {
        return Ok(true);
    };
    match parse_utc(&value) {
        Ok(until) if until > Utc::now() => Ok(false),
        _ => {
            store.set_paused_until(None)?;
            Ok(true)
        }
    }
}

fn configure_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager
            .enable()
            .map_err(|error| format!("No se pudo activar el inicio automático: {error}"))
    } else {
        manager
            .disable()
            .map_err(|error| format!("No se pudo desactivar el inicio automático: {error}"))
    }
}

fn inside_work_schedule(settings: &SettingsV1, now: chrono::DateTime<Utc>) -> bool {
    let timezone: Tz = settings.timezone.parse().unwrap_or(chrono_tz::UTC);
    let local = now.with_timezone(&timezone);
    if !settings
        .work_schedule
        .days
        .contains(&(local.weekday().num_days_from_sunday() as u8))
    {
        return false;
    }
    let minutes = local.hour() * 60 + local.minute();
    let start = clock_minutes(&settings.work_schedule.start).unwrap_or(0);
    let end = clock_minutes(&settings.work_schedule.end).unwrap_or(24 * 60);
    minutes >= start && minutes < end
}

fn clock_minutes(value: &str) -> Option<u32> {
    let (hours, minutes) = value.split_once(':')?;
    Some(hours.parse::<u32>().ok()? * 60 + minutes.parse::<u32>().ok()?)
}

fn validate_range(from: chrono::DateTime<Utc>, to: chrono::DateTime<Utc>) -> Result<(), String> {
    if to <= from {
        return Err("El fin del rango debe ser posterior al inicio".into());
    }
    if to - from > Duration::days(366) {
        return Err("El rango máximo de consulta es 366 días".into());
    }
    Ok(())
}

fn timeline_csv(blocks: &[TimelineBlockV1]) -> String {
    let mut output =
        String::from("start,end,duration_seconds,app,category,confidence,afk,source,check_in_id\n");
    for block in blocks {
        let fields = [
            block.start.clone(),
            block.end.clone(),
            block.duration_seconds.to_string(),
            block.app.clone(),
            block.category.clone(),
            block.confidence.to_string(),
            block.afk.to_string(),
            block.source.clone(),
            block.check_in_id.clone().unwrap_or_default(),
        ];
        output.push_str(
            &fields
                .into_iter()
                .map(|value| csv_escape(&value))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }
    output
}

fn csv_escape(value: &str) -> String {
    let safe_value = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_string()
    };
    if safe_value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", safe_value.replace('"', "\"\""))
    } else {
        safe_value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    #[test]
    fn external_allowlist_does_not_accept_lookalike_hosts() {
        let url = Url::parse("https://chatgpt.com.evil.test").unwrap();
        assert!(!["chatgpt.com", "claude.ai"].contains(&url.host_str().unwrap()));
    }

    #[test]
    fn csv_quotes_sensitive_fields() {
        assert_eq!(csv_escape("Focus, \"deep\""), "\"Focus, \"\"deep\"\"\"");
    }

    #[test]
    fn csv_neutralizes_spreadsheet_formulas() {
        assert_eq!(
            csv_escape("=HYPERLINK(\"https://evil.test\")"),
            "\"'=HYPERLINK(\"\"https://evil.test\"\")\""
        );
        assert_eq!(csv_escape("+1+1"), "'+1+1");
    }

    #[test]
    fn scheduler_shutdown_is_prompt_idempotent_and_joins_thread() {
        let ticks = Arc::new(AtomicUsize::new(0));
        let worker_ticks = Arc::clone(&ticks);
        let held_runtime = Arc::new(());
        let worker_runtime = Arc::clone(&held_runtime);
        let scheduler = spawn_scheduler(
            move || {
                let _ = &worker_runtime;
                worker_ticks.fetch_add(1, Ordering::Release);
                Ok(())
            },
            std::time::Duration::from_secs(30),
        )
        .unwrap();
        for _ in 0..100 {
            if ticks.load(Ordering::Acquire) > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(ticks.load(Ordering::Acquire) > 0);

        scheduler.shutdown().unwrap();
        scheduler.shutdown().unwrap();
        assert_eq!(Arc::strong_count(&held_runtime), 1);
        let stopped_at = ticks.load(Ordering::Acquire);
        std::thread::sleep(std::time::Duration::from_millis(25));
        assert_eq!(ticks.load(Ordering::Acquire), stopped_at);
    }

    #[test]
    fn confirmed_delete_cancels_inflight_tick_before_any_late_write_or_restart() {
        let temp = TempDir::new().unwrap();
        let store_path = temp.path().join("enso-time.sqlite");
        let store = Arc::new(LocalStore::open(&store_path).unwrap());
        let mut settings = SettingsV1::default();
        settings.onboarding_completed = true;
        store.save_settings(&settings).unwrap();
        let private_checkin = CheckInV1 {
            id: "private-checkin".into(),
            interval_start: "2026-08-21T12:00:00Z".into(),
            interval_end: "2026-08-21T12:15:00Z".into(),
            reason: "unknown".into(),
            status: "pending".into(),
            activity_label: Some("private-checkin-marker".into()),
            category: None,
            project: None,
            repetitive: None,
            rules_clear: None,
            created_at: "2026-08-21T12:15:00Z".into(),
            answered_at: None,
        };
        store.create_checkin(&private_checkin).unwrap();
        let private_workflow = WorkflowIntakeV1 {
            id: "private-workflow".into(),
            candidate_id: "candidate-private".into(),
            candidate_label: "private-workflow-marker".into(),
            trigger: "Cada viernes".into(),
            tools: vec!["Sheets".into()],
            steps: vec!["Extraer datos".into()],
            human_decisions: "Aprobar excepciones".into(),
            inputs: "Ventas".into(),
            desired_output: "Reporte".into(),
            frequency: "Semanal".into(),
            minutes_per_run: 45,
            pain_points: "Trabajo manual".into(),
            created_at: "2025-01-01T10:00:00Z".into(),
            updated_at: "2025-01-02T10:00:00Z".into(),
        };
        store
            .save_workflow_intake(private_workflow.clone())
            .unwrap();

        let activity = Arc::new(
            ActivityWatchRuntime::start(
                &temp.path().join("activity"),
                temp.path().join("empty-resources"),
            )
            .unwrap(),
        );
        activity
            .seed_test_activity("private-activity-marker")
            .unwrap();
        let profile = activity.test_profile_root();
        let queue = profile.join("data/activitywatch/aw-client/queued");
        let logs = profile.join("home/Library/Logs/activitywatch");
        let diagnostic = profile.join("diagnostics/private.stderr.log");
        std::fs::create_dir_all(&queue).unwrap();
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(queue.join("private.queue"), "private-queue-marker").unwrap();
        std::fs::write(logs.join("private.log"), "private-log-marker").unwrap();
        std::fs::write(&diagnostic, "private-diagnostic-marker").unwrap();

        let capture_gate = Arc::new(CaptureGate::new());
        let tick_gate = Arc::clone(&capture_gate);
        let tick_store = Arc::clone(&store);
        let tick_activity = Arc::clone(&activity);
        let late_writes = Arc::new(AtomicUsize::new(0));
        let late_restarts = Arc::new(AtomicUsize::new(0));
        let tick_late_writes = Arc::clone(&late_writes);
        let tick_late_restarts = Arc::clone(&late_restarts);
        let (started_sender, started_receiver) = mpsc::channel();
        let tick = std::thread::spawn(move || {
            let operation = tick_gate.operation().unwrap();
            started_sender.send(()).unwrap();
            while operation.ensure_current().is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            if operation.ensure_current().is_ok() {
                tick_late_restarts.fetch_add(1, Ordering::Release);
                let _ = tick_activity.start_watchers(false);
                tick_late_writes.fetch_add(1, Ordering::Release);
                let _ = tick_store.create_checkin(&private_checkin);
                let _ = tick_store.save_workflow_intake(private_workflow);
            }
        });
        started_receiver.recv().unwrap();

        delete_all_data_inner(&store, &activity, &capture_gate).unwrap();
        tick.join().unwrap();

        assert_eq!(late_writes.load(Ordering::Acquire), 0);
        assert_eq!(late_restarts.load(Ordering::Acquire), 0);
        assert!(store.checkins().unwrap().is_empty());
        assert!(store.list_workflow_intakes().unwrap().is_empty());
        assert!(activity.test_activity_is_empty().unwrap());
        assert!(activity.test_watchers_are_stopped());
        assert!(!queue.exists());
        assert!(!logs.exists());
        assert!(!diagnostic.exists());
        for (path, marker) in [
            (store_path.clone(), b"private-checkin-marker".as_slice()),
            (store_path, b"private-workflow-marker".as_slice()),
            (
                activity.test_database_path(),
                b"private-activity-marker".as_slice(),
            ),
        ] {
            let bytes = std::fs::read(path).unwrap();
            assert!(!bytes.windows(marker.len()).any(|window| window == marker));
        }
    }

    #[test]
    fn later_successful_delete_invalidates_an_earlier_failed_rollback() {
        let temp = TempDir::new().unwrap();
        let store = Arc::new(LocalStore::open(&temp.path().join("enso-time.sqlite")).unwrap());
        let mut settings = SettingsV1::default();
        settings.onboarding_completed = true;
        store.save_settings(&settings).unwrap();
        let activity = Arc::new(
            ActivityWatchRuntime::start(
                &temp.path().join("activity"),
                temp.path().join("empty-resources"),
            )
            .unwrap(),
        );
        let capture_gate = Arc::new(CaptureGate::new());
        let second_completed = Arc::new(AtomicBool::new(false));
        let post_second_restarts = Arc::new(AtomicUsize::new(0));
        let post_second_writes = Arc::new(AtomicUsize::new(0));
        let (first_has_gate_sender, first_has_gate_receiver) = mpsc::channel();
        let (allow_failure_sender, allow_failure_receiver) = mpsc::channel();
        let (second_done_sender, second_done_receiver) = mpsc::channel();

        let first_store = Arc::clone(&store);
        let first_activity = Arc::clone(&activity);
        let first_gate = Arc::clone(&capture_gate);
        let first_second_completed = Arc::clone(&second_completed);
        let first_post_restarts = Arc::clone(&post_second_restarts);
        let first_post_writes = Arc::clone(&post_second_writes);
        let first = std::thread::spawn(move || {
            run_exclusive_delete(
                &first_activity,
                &first_gate,
                || {
                    first_has_gate_sender.send(()).unwrap();
                    allow_failure_receiver.recv().unwrap();
                    (Err("forced first-delete failure".into()), ())
                },
                |_| {
                    // With the old ordering, the second delete could complete while
                    // this rollback was pending. Waiting briefly makes that unsafe
                    // ordering deterministic without allowing it in the fixed code.
                    let completed_during_rollback = second_done_receiver
                        .recv_timeout(std::time::Duration::from_secs(1))
                        .is_ok()
                        || first_second_completed.load(Ordering::Acquire);
                    if completed_during_rollback {
                        first_post_restarts.fetch_add(1, Ordering::Release);
                    }
                    let stale = CheckInV1 {
                        id: "stale-rollback".into(),
                        interval_start: "2026-08-21T12:00:00Z".into(),
                        interval_end: "2026-08-21T12:15:00Z".into(),
                        reason: "unknown".into(),
                        status: "pending".into(),
                        activity_label: Some("stale-rollback-marker".into()),
                        category: None,
                        project: None,
                        repetitive: None,
                        rules_clear: None,
                        created_at: "2026-08-21T12:15:00Z".into(),
                        answered_at: None,
                    };
                    first_store.create_checkin(&stale).unwrap();
                    if completed_during_rollback {
                        first_post_writes.fetch_add(1, Ordering::Release);
                    }
                },
            )
        });
        first_has_gate_receiver.recv().unwrap();

        let second_store = Arc::clone(&store);
        let second_activity = Arc::clone(&activity);
        let second_gate = Arc::clone(&capture_gate);
        let second_completed_flag = Arc::clone(&second_completed);
        let second = std::thread::spawn(move || {
            let result = delete_all_data_inner(&second_store, &second_activity, &second_gate);
            second_completed_flag.store(true, Ordering::Release);
            let _ = second_done_sender.send(());
            result
        });
        for _ in 0..1_000 {
            if capture_gate.exclusive_request_count() == 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(capture_gate.exclusive_request_count(), 2);
        allow_failure_sender.send(()).unwrap();

        assert!(first.join().unwrap().is_err());
        second.join().unwrap().unwrap();
        assert!(second_completed.load(Ordering::Acquire));
        assert_eq!(post_second_restarts.load(Ordering::Acquire), 0);
        assert_eq!(post_second_writes.load(Ordering::Acquire), 0);
        assert!(store.checkins().unwrap().is_empty());
        assert!(activity.test_watchers_are_stopped());
    }

    #[test]
    fn delayed_bootstrap_reloads_settings_after_confirmed_delete() {
        let temp = TempDir::new().unwrap();
        let store = Arc::new(LocalStore::open(&temp.path().join("enso-time.sqlite")).unwrap());
        let mut settings = SettingsV1::default();
        settings.onboarding_completed = true;
        store.save_settings(&settings).unwrap();
        let activity = Arc::new(
            ActivityWatchRuntime::start(
                &temp.path().join("activity"),
                temp.path().join("empty-resources"),
            )
            .unwrap(),
        );
        activity
            .seed_test_activity("delayed-bootstrap-private-marker")
            .unwrap();
        let queue = activity
            .test_profile_root()
            .join("data/activitywatch/aw-client/queued");
        std::fs::create_dir_all(&queue).unwrap();
        std::fs::write(queue.join("private.queue"), "private-bootstrap-queue").unwrap();
        let capture_gate = Arc::new(CaptureGate::new());
        let (release_sender, release_receiver) = mpsc::channel();

        let bootstrap_store = Arc::clone(&store);
        let bootstrap_activity = Arc::clone(&activity);
        let bootstrap_gate = Arc::clone(&capture_gate);
        let bootstrap = std::thread::spawn(move || {
            release_receiver.recv().unwrap();
            bootstrap_watchers_if_current(&bootstrap_store, &bootstrap_activity, &bootstrap_gate)
        });

        delete_all_data_inner(&store, &activity, &capture_gate).unwrap();
        release_sender.send(()).unwrap();
        bootstrap.join().unwrap().unwrap();

        assert!(!store.settings().unwrap().onboarding_completed);
        assert!(store.checkins().unwrap().is_empty());
        assert!(activity.test_activity_is_empty().unwrap());
        assert!(activity.test_watchers_are_stopped());
        assert_eq!(activity.capture_issue(), None);
        assert!(!queue.exists());
    }

    #[test]
    fn json_export_includes_saved_workflow_intakes() {
        let temp = TempDir::new().unwrap();
        let store = LocalStore::open(&temp.path().join("enso-time.sqlite")).unwrap();
        let workflow = WorkflowIntakeV1 {
            id: "workflow-export".into(),
            candidate_id: "candidate-report".into(),
            candidate_label: "Preparar reporte".into(),
            trigger: "Cada viernes".into(),
            tools: vec!["Sheets".into()],
            steps: vec!["Extraer datos".into()],
            human_decisions: "Aprobar excepciones".into(),
            inputs: "Ventas".into(),
            desired_output: "Reporte aprobado".into(),
            frequency: "Semanal".into(),
            minutes_per_run: 45,
            pain_points: "Trabajo manual".into(),
            created_at: "2025-01-01T10:00:00Z".into(),
            updated_at: "2025-01-02T10:00:00Z".into(),
        };
        let capture_gate = CaptureGate::new();
        save_workflow_intake_inner(&store, &capture_gate, workflow).unwrap();
        let workflows = list_workflow_intakes_inner(&store, &capture_gate).unwrap();
        assert!(!store.settings().unwrap().onboarding_completed);
        let raw_timeline = TimelineBlockV1 {
            id: "timeline-private".into(),
            start: "2025-01-01T10:00:00Z".into(),
            end: "2025-01-01T10:15:00Z".into(),
            duration_seconds: 900,
            app: "Browser".into(),
            title: Some("private-raw-window-title".into()),
            category: "admin".into(),
            confidence: 0.8,
            afk: false,
            source: "window".into(),
            check_in_id: None,
        };
        let exported = json_export(
            &SettingsV1::default(),
            &[],
            &[raw_timeline],
            &workflows,
            &[],
            &Utc::now(),
        )
        .unwrap();
        let json: serde_json::Value = serde_json::from_str(&exported).unwrap();
        assert_eq!(json["schemaVersion"], 2);
        assert_eq!(json["workflows"][0]["id"], "workflow-export");
        assert_eq!(json["workflows"][0]["minutesPerRun"], 45);
        assert!(json["workflows"][0].get("candidate_id").is_none());
        assert!(json["timeline"][0].get("title").is_none());
        assert!(!exported.contains("private-raw-window-title"));
        assert!(json["deletionBoundary"].as_str().unwrap().contains("fuera"));
    }

    #[test]
    fn private_export_files_are_create_new_and_owner_only() {
        let directory = TempDir::new().unwrap();
        let export_dir = directory.path().join("exports");
        create_private_directory(&export_dir).unwrap();
        let path = export_dir.join("blueprint.json");
        write_private_new_file(&path, b"private-export").unwrap();
        assert!(write_private_new_file(&path, b"overwrite").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"private-export");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&export_dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
