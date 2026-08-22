use crate::activitywatch::{completed_interval, ActivityWatchRuntime};
use crate::diagnostics::create_diagnostic;
use crate::models::{
    iso_now, parse_utc, CheckInAnswer, CheckInV1, DiagnosticV1, ExportEnvelope, SettingsV1,
    StudyProgress, TimelineBlockV1, TrackingStatus, UpdateInfo,
};
use crate::storage::LocalStore;
use chrono::{Datelike, Duration, Timelike, Utc};
use chrono_tz::Tz;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use url::Url;
use uuid::Uuid;

pub struct AppState {
    pub store: Arc<LocalStore>,
    pub activity: Arc<ActivityWatchRuntime>,
    pub app_data_dir: PathBuf,
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
    let previous = state.store.settings()?;
    settings.validate()?;
    configure_autostart(&app, settings.onboarding_completed && settings.autostart)?;
    state.store.save_settings(&settings)?;
    if previous.onboarding_completed && !settings.onboarding_completed {
        state.activity.stop_watchers()?;
    } else if !previous.onboarding_completed && settings.onboarding_completed {
        state
            .activity
            .start_watchers(settings.capture_window_titles)?;
    } else if previous.capture_window_titles != settings.capture_window_titles
        && is_running(&state.store)?
    {
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
    let blocks = state.activity.timeline(start, now, &settings, &checkins)?;
    let observed = ((now - start).num_seconds().max(0) as f64 / 86_400.0).min(f64::from(days));
    Ok(create_diagnostic(&blocks, &checkins, observed, &settings))
}

#[tauri::command]
pub fn pause_tracking(state: State<'_, AppState>, minutes: u16) -> Result<TrackingStatus, String> {
    if !(5..=1440).contains(&minutes) {
        return Err("La pausa debe durar entre 5 minutos y 24 horas".into());
    }
    let paused_until = Utc::now() + Duration::minutes(i64::from(minutes));
    state.activity.stop_watchers()?;
    state
        .store
        .set_paused_until(Some(&paused_until.to_rfc3339()))?;
    status(&state)
}

#[tauri::command]
pub fn resume_tracking(state: State<'_, AppState>) -> Result<TrackingStatus, String> {
    let settings = state.store.settings()?;
    if !settings.onboarding_completed {
        return Err("Completa la configuración inicial antes de iniciar el seguimiento".into());
    }
    state
        .activity
        .start_watchers(settings.capture_window_titles)?;
    state.store.set_paused_until(None)?;
    status(&state)
}

#[tauri::command]
pub fn export_data(state: State<'_, AppState>, format: String) -> Result<String, String> {
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
    let timeline = state.activity.timeline(start, now, &settings, &checkins)?;
    let export_dir = state.app_data_dir.join("exports");
    std::fs::create_dir_all(&export_dir)
        .map_err(|error| format!("No se pudo crear el directorio de exportación: {error}"))?;
    let filename = format!("enso-time-{}.{}", now.format("%Y%m%d-%H%M%S"), format);
    let path = export_dir.join(filename);
    let contents = if format == "json" {
        serde_json::to_string_pretty(&ExportEnvelope {
            schema_version: 1,
            exported_at: now.to_rfc3339(),
            settings,
            check_ins: checkins,
            timeline,
        })
        .map_err(|error| format!("No se pudo preparar la exportación: {error}"))?
    } else {
        timeline_csv(&timeline)
    };
    std::fs::write(&path, contents)
        .map_err(|error| format!("No se pudo escribir la exportación: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("No se pudo proteger la exportación: {error}"))?;
    }
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn delete_all_data(state: State<'_, AppState>, confirmation: String) -> Result<(), String> {
    if confirmation != "DELETE_ENSO_TIME_DATA" {
        return Err("La confirmación de borrado no coincide".into());
    }
    state.activity.stop_watchers()?;
    if let Err(error) = state.activity.delete_queued_activity() {
        let settings = state.store.settings()?;
        let _ = state
            .activity
            .start_watchers(settings.capture_window_titles);
        return Err(error);
    }
    if let Err(error) = state.activity.delete_all_activity() {
        let settings = state.store.settings()?;
        let _ = state
            .activity
            .start_watchers(settings.capture_window_titles);
        return Err(format!(
            "No se borraron los datos porque el almacén de actividad falló: {error}"
        ));
    }
    state.store.delete_all()?;
    let settings = state.store.settings()?;
    if settings.onboarding_completed {
        state
            .activity
            .start_watchers(settings.capture_window_titles)?;
    }
    Ok(())
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
) {
    std::thread::Builder::new()
        .name("enso-checkin-scheduler".into())
        .spawn(move || loop {
            if let Err(error) = scheduler_tick(&app, &store, &activity) {
                eprintln!("Ensō Time scheduler: {error}");
            }
            std::thread::sleep(std::time::Duration::from_secs(30));
        })
        .expect("failed to spawn Ensō Time scheduler");
}

fn scheduler_tick(
    app: &AppHandle,
    store: &LocalStore,
    activity: &ActivityWatchRuntime,
) -> Result<(), String> {
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
    let should_run = is_running(&state.store)?;
    let paused_until = state.store.paused_until()?;
    if should_run {
        let settings = state.store.settings()?;
        let (window, afk) = state.activity.watcher_statuses();
        if window == "stopped" || afk == "stopped" {
            let _ = state
                .activity
                .start_watchers(settings.capture_window_titles);
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
    let mut output = String::from(
        "start,end,duration_seconds,app,title,category,confidence,afk,source,check_in_id\n",
    );
    for block in blocks {
        let fields = [
            block.start.clone(),
            block.end.clone(),
            block.duration_seconds.to_string(),
            block.app.clone(),
            block.title.clone().unwrap_or_default(),
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
}
