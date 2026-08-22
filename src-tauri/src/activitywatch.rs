use crate::models::{AppRule, CheckInV1, SettingsV1, TimelineBlockV1};
use aw_datastore::Datastore;
use aw_models::Event;
use chrono::{DateTime, Duration, Utc};
use std::net::{Ipv4Addr, TcpListener};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;
use uuid::Uuid;

const PORT_START: u16 = 5610;
const PORT_END: u16 = 5699;

pub struct ActivityWatchRuntime {
    datastore: Datastore,
    port: AtomicU16,
    server_ready: Arc<AtomicBool>,
    server_failed: Arc<Mutex<Option<String>>>,
    window_watcher: Mutex<Option<Child>>,
    afk_watcher: Mutex<Option<Child>>,
    resource_dir: PathBuf,
    profile_data_dir: PathBuf,
}

impl ActivityWatchRuntime {
    pub fn start(
        data_dir: &Path,
        resource_dir: PathBuf,
        profile_config_base: &Path,
        profile_data_base: &Path,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir)
            .map_err(|error| format!("No se pudo crear el almacén de actividad: {error}"))?;
        let database = data_dir.join("activitywatch.sqlite");
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            std::fs::set_permissions(data_dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|error| format!("No se pudo proteger el almacén de actividad: {error}"))?;
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .mode(0o600)
                .open(&database)
                .map_err(|error| format!("No se pudo preparar el almacén de actividad: {error}"))?;
        }
        let database = database
            .to_str()
            .ok_or_else(|| "La ruta del almacén de actividad no es válida".to_string())?
            .to_string();
        let port = available_port()?;
        let api_key = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        write_watcher_server_config(profile_config_base, port, &api_key)?;
        let datastore = Datastore::new(database, false);
        let server_datastore = datastore.clone();
        let server_ready = Arc::new(AtomicBool::new(false));
        let runtime = Self {
            datastore,
            port: AtomicU16::new(port),
            server_ready: Arc::clone(&server_ready),
            server_failed: Arc::new(Mutex::new(None)),
            window_watcher: Mutex::new(None),
            afk_watcher: Mutex::new(None),
            resource_dir,
            profile_data_dir: profile_data_base.join("activitywatch-enso-time"),
        };
        let failed = runtime.server_failed.clone();
        // Rocket owns its async runtime. It is isolated from Tauri so a local capture
        // failure cannot take down the UI thread.
        std::thread::Builder::new()
            .name("enso-aw-server".into())
            .spawn(move || {
                let result = run_server(server_datastore, port, api_key, server_ready);
                if let Err(error) = result {
                    if let Ok(mut slot) = failed.lock() {
                        *slot = Some(error);
                    }
                }
            })
            .map_err(|error| format!("No se pudo iniciar el servicio local: {error}"))?;
        Ok(runtime)
    }

    pub fn port_if_ready(&self) -> Option<u16> {
        if self.server_error().is_some() {
            return None;
        }
        self.server_ready
            .load(Ordering::Acquire)
            .then(|| self.port.load(Ordering::Relaxed))
    }

    pub fn server_error(&self) -> Option<String> {
        self.server_failed
            .lock()
            .ok()
            .and_then(|value| value.clone())
    }

    pub fn start_watchers(&self, capture_titles: bool) -> Result<(), String> {
        let port = self.ready_port()?;
        self.start_watcher(WatcherKind::Window, port, capture_titles)?;
        if let Err(error) = self.start_watcher(WatcherKind::Afk, port, capture_titles) {
            let _ = terminate_slot(&self.window_watcher);
            return Err(error);
        }
        Ok(())
    }

    pub fn restart_window_watcher(&self, capture_titles: bool) -> Result<(), String> {
        // Never stop a healthy watcher until Rocket has confirmed that the private
        // loopback endpoint is ours. This also closes the bind/start TOCTOU window.
        let port = self.ready_port()?;
        terminate_slot(&self.window_watcher)?;
        self.start_watcher(WatcherKind::Window, port, capture_titles)
    }

    fn ready_port(&self) -> Result<u16, String> {
        self.port_if_ready().ok_or_else(|| {
            self.server_error().unwrap_or_else(|| {
                "El servicio local todavía no está listo; se reintentará automáticamente".into()
            })
        })
    }

    pub fn stop_watchers(&self) -> Result<(), String> {
        let window_result = terminate_slot(&self.window_watcher);
        let afk_result = terminate_slot(&self.afk_watcher);
        window_result.and(afk_result)
    }

    pub fn watcher_statuses(&self) -> (String, String) {
        (
            watcher_status(
                &self.window_watcher,
                self.binary_path(WatcherKind::Window).exists(),
            ),
            watcher_status(
                &self.afk_watcher,
                self.binary_path(WatcherKind::Afk).exists(),
            ),
        )
    }

    pub fn timeline(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        settings: &SettingsV1,
        checkins: &[CheckInV1],
    ) -> Result<Vec<TimelineBlockV1>, String> {
        if to <= from {
            return Err("El fin del rango debe ser posterior al inicio".into());
        }
        let buckets = safe_datastore(|| self.datastore.get_buckets())?;
        let window_buckets: Vec<_> = buckets
            .values()
            .filter(|bucket| bucket._type == "currentwindow")
            .collect();
        let afk_buckets: Vec<_> = buckets
            .values()
            .filter(|bucket| bucket._type == "afkstatus")
            .collect();

        let mut afk_events = Vec::new();
        for bucket in afk_buckets {
            afk_events.extend(safe_datastore(|| {
                self.datastore
                    .get_events(&bucket.id, Some(from), Some(to), None)
            })?);
        }

        let mut blocks = Vec::new();
        for bucket in window_buckets {
            let events = safe_datastore(|| {
                self.datastore
                    .get_events(&bucket.id, Some(from), Some(to), None)
            })?;
            for event in events {
                let event_end = event.calculate_endtime();
                let app =
                    string_field(&event, "app").unwrap_or_else(|| "Aplicación desconocida".into());
                let title = if settings.capture_window_titles {
                    string_field(&event, "title")
                } else {
                    None
                };
                let matched_checkin = checkins.iter().find(|checkin| {
                    checkin.status == "answered"
                        && overlaps_strings(
                            &checkin.interval_start,
                            &checkin.interval_end,
                            event.timestamp,
                            event_end,
                        )
                });
                let (category, source, check_in_id, confidence) =
                    if let Some(checkin) = matched_checkin {
                        (
                            checkin.category.clone().unwrap_or_else(|| "unknown".into()),
                            "checkin".into(),
                            Some(checkin.id.clone()),
                            1.0,
                        )
                    } else {
                        let (category, confidence) =
                            classify(&app, title.as_deref(), &settings.app_rules);
                        (category, "window".into(), None, confidence)
                    };
                let is_afk = afk_events.iter().any(|afk| {
                    string_field(afk, "status").as_deref() == Some("afk")
                        && ranges_overlap(
                            event.timestamp,
                            event_end,
                            afk.timestamp,
                            afk.calculate_endtime(),
                        )
                });
                blocks.push(TimelineBlockV1 {
                    id: format!(
                        "{}-{}",
                        bucket.id,
                        event
                            .id
                            .unwrap_or_else(|| event.timestamp.timestamp_millis())
                    ),
                    start: event.timestamp.to_rfc3339(),
                    end: event_end.to_rfc3339(),
                    duration_seconds: event.duration.num_seconds().max(0),
                    app,
                    title,
                    category: if is_afk { "break".into() } else { category },
                    confidence,
                    afk: is_afk,
                    source,
                    check_in_id,
                });
            }
        }

        for checkin in checkins.iter().filter(|item| item.status == "answered") {
            let covered = blocks
                .iter()
                .any(|block| block.check_in_id.as_ref() == Some(&checkin.id));
            if !covered {
                let start = crate::models::parse_utc(&checkin.interval_start)?;
                let end = crate::models::parse_utc(&checkin.interval_end)?;
                if ranges_overlap(start, end, from, to) {
                    blocks.push(TimelineBlockV1 {
                        id: format!("manual-{}", checkin.id),
                        start: start.max(from).to_rfc3339(),
                        end: end.min(to).to_rfc3339(),
                        duration_seconds: (end.min(to) - start.max(from)).num_seconds().max(0),
                        app: "Contexto manual".into(),
                        title: checkin.activity_label.clone(),
                        category: checkin.category.clone().unwrap_or_else(|| "unknown".into()),
                        confidence: 1.0,
                        afk: false,
                        source: "checkin".into(),
                        check_in_id: Some(checkin.id.clone()),
                    });
                }
            }
        }
        blocks.sort_by(|left, right| left.start.cmp(&right.start));
        Ok(blocks)
    }

    pub fn interval_reason(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        settings: &SettingsV1,
    ) -> Result<Option<String>, String> {
        let blocks = self.timeline(from, to, settings, &[])?;
        let interval_seconds = (to - from).num_seconds().max(1);
        let tracked: i64 = blocks.iter().map(|block| block.duration_seconds).sum();
        let afk: i64 = blocks
            .iter()
            .filter(|block| block.afk)
            .map(|block| block.duration_seconds)
            .sum();
        if tracked < interval_seconds * 3 / 5 {
            return Ok(Some("unknown".into()));
        }
        if afk > interval_seconds / 2 {
            return Ok(Some("afk".into()));
        }
        let ambiguous: i64 = blocks
            .iter()
            .filter(|block| block.category == "unknown" || block.confidence < 0.5)
            .map(|block| block.duration_seconds)
            .sum();
        if ambiguous > interval_seconds / 2 {
            return Ok(Some("ambiguous".into()));
        }
        Ok(None)
    }

    pub fn latest_event_at(&self) -> Result<Option<String>, String> {
        let buckets = safe_datastore(|| self.datastore.get_buckets())?;
        Ok(buckets
            .values()
            .filter_map(|bucket| bucket.metadata.end)
            .max()
            .map(|date| date.to_rfc3339()))
    }

    pub fn delete_all_activity(&self) -> Result<(), String> {
        let buckets = safe_datastore(|| self.datastore.get_buckets())?;
        for bucket_id in buckets.keys() {
            safe_datastore(|| self.datastore.delete_bucket(bucket_id))?;
        }
        safe_datastore(|| self.datastore.force_commit())
    }

    pub fn delete_queued_activity(&self) -> Result<(), String> {
        let queue = self.profile_data_dir.join("aw-client").join("queued");
        if queue.exists() {
            std::fs::remove_dir_all(&queue)
                .map_err(|error| format!("No se pudo borrar la cola privada: {error}"))?;
        }
        Ok(())
    }

    pub fn prune_activity(&self, retention_days: Option<u16>) -> Result<usize, String> {
        let Some(days) = retention_days else {
            return Ok(0);
        };
        let cutoff = Utc::now() - Duration::days(i64::from(days));
        let buckets = safe_datastore(|| self.datastore.get_buckets())?;
        let mut deleted = 0;
        for bucket_id in buckets.keys() {
            let events = safe_datastore(|| {
                self.datastore
                    .get_events_unclipped(bucket_id, None, Some(cutoff), None)
            })?;
            let event_ids: Vec<i64> = events
                .iter()
                .filter(|event| event.calculate_endtime() <= cutoff)
                .filter_map(|event| event.id)
                .collect();
            if !event_ids.is_empty() {
                deleted += event_ids.len();
                safe_datastore(|| self.datastore.delete_events_by_id(bucket_id, event_ids))?;
            }
        }
        if deleted > 0 {
            safe_datastore(|| self.datastore.force_commit())?;
        }
        Ok(deleted)
    }

    fn start_watcher(
        &self,
        kind: WatcherKind,
        port: u16,
        capture_titles: bool,
    ) -> Result<(), String> {
        let slot = match kind {
            WatcherKind::Window => &self.window_watcher,
            WatcherKind::Afk => &self.afk_watcher,
        };
        if matches!(
            watcher_status(slot, self.binary_path(kind).exists()).as_str(),
            "running"
        ) {
            return Ok(());
        }
        let binary = self.binary_path(kind);
        if !binary.is_file() {
            return Err(format!(
                "No se encontró el capturador {} en {}",
                kind.label(),
                binary.display()
            ));
        }
        let mut command = Command::new(&binary);
        command
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .env("AW_PROFILE", "enso-time")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if matches!(kind, WatcherKind::Window) && !capture_titles {
            command.arg("--exclude-title");
        }
        let child = command.spawn().map_err(|error| {
            format!("No se pudo iniciar el capturador {}: {error}", kind.label())
        })?;
        *slot
            .lock()
            .map_err(|_| "No se pudo actualizar el estado del capturador")? = Some(child);
        Ok(())
    }

    fn binary_path(&self, kind: WatcherKind) -> PathBuf {
        let name = kind.binary_name();
        let directory = self.resource_dir.join("activitywatch").join(name);
        #[cfg(target_os = "windows")]
        let binary = directory.join(format!("{name}.exe"));
        #[cfg(not(target_os = "windows"))]
        let binary = directory.join(name);
        binary
    }
}

impl Drop for ActivityWatchRuntime {
    fn drop(&mut self) {
        let _ = self.stop_watchers();
        self.datastore.close();
    }
}

#[derive(Clone, Copy)]
enum WatcherKind {
    Window,
    Afk,
}

impl WatcherKind {
    fn binary_name(self) -> &'static str {
        match self {
            Self::Window => "aw-watcher-window",
            Self::Afk => "aw-watcher-afk",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Window => "de ventanas",
            Self::Afk => "de inactividad",
        }
    }
}

fn run_server(
    datastore: Datastore,
    port: u16,
    api_key: String,
    ready: Arc<AtomicBool>,
) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("No se pudo crear el runtime local: {error}"))?;
    runtime.block_on(async move {
        let mut config = aw_server::config::AWConfig::default();
        config.address = "127.0.0.1".into();
        config.port = port;
        config.testing = false;
        config.cors.clear();
        config.cors_regex.clear();
        config.auth.api_key = Some(api_key);
        let state = aw_server::endpoints::ServerState {
            datastore: Mutex::new(datastore),
            asset_resolver: aw_server::endpoints::AssetResolver::new(None),
            device_id: "enso-time-local".into(),
        };
        let ready_fairing = rocket::fairing::AdHoc::on_liftoff("Ensō server ready", move |_| {
            let ready = Arc::clone(&ready);
            Box::pin(async move {
                ready.store(true, Ordering::Release);
            })
        });
        aw_server::endpoints::build_rocket(state, config)
            .attach(ready_fairing)
            .launch()
            .await
            .map(|_| ())
            .map_err(|error| format!("El servicio ActivityWatch se detuvo: {error}"))
    })
}

fn write_watcher_server_config(base: &Path, port: u16, api_key: &str) -> Result<(), String> {
    let directory = base.join("activitywatch-enso-time").join("aw-server-rust");
    std::fs::create_dir_all(&directory).map_err(|error| {
        format!("No se pudo crear la configuración privada del capturador: {error}")
    })?;
    let path = directory.join("config.toml");
    let contents =
        format!("address = \"127.0.0.1\"\nport = {port}\n\n[auth]\napi_key = \"{api_key}\"\n");
    std::fs::write(&path, contents)
        .map_err(|error| format!("No se pudo proteger el servicio local: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .and_then(|_| std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)))
            .map_err(|error| format!("No se pudo restringir la configuración local: {error}"))?;
    }
    Ok(())
}

fn available_port() -> Result<u16, String> {
    for port in PORT_START..=PORT_END {
        if TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok() {
            return Ok(port);
        }
    }
    Err("No hay un puerto local libre entre 5610 y 5699".into())
}

fn safe_datastore<T, F>(operation: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, aw_datastore::DatastoreError>,
{
    catch_unwind(AssertUnwindSafe(operation))
        .map_err(|_| "El almacén de actividad dejó de responder".to_string())?
        .map_err(|error| format!("Error del almacén de actividad: {error:?}"))
}

fn watcher_status(slot: &Mutex<Option<Child>>, binary_exists: bool) -> String {
    let Ok(mut guard) = slot.lock() else {
        return "error".into();
    };
    let Some(child) = guard.as_mut() else {
        return if binary_exists { "stopped" } else { "missing" }.into();
    };
    match child.try_wait() {
        Ok(None) => "running".into(),
        Ok(Some(_)) => {
            *guard = None;
            "stopped".into()
        }
        Err(_) => "error".into(),
    }
}

fn terminate_slot(slot: &Mutex<Option<Child>>) -> Result<(), String> {
    let mut guard = slot
        .lock()
        .map_err(|_| "No se pudo detener el capturador")?;
    let Some(mut child) = guard.take() else {
        return Ok(());
    };
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        let _ = kill(Pid::from_raw(child.id() as i32), Signal::SIGTERM);
        for _ in 0..20 {
            if child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_some()
            {
                return Ok(());
            }
            std::thread::sleep(StdDuration::from_millis(50));
        }
    }
    child
        .kill()
        .map_err(|error| format!("No se pudo detener el capturador: {error}"))?;
    child.wait().map(|_| ()).map_err(|error| error.to_string())
}

fn string_field(event: &Event, key: &str) -> Option<String> {
    event
        .data
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

fn classify(app: &str, title: Option<&str>, rules: &[AppRule]) -> (String, f64) {
    let searchable = format!("{} {}", app, title.unwrap_or_default()).to_lowercase();
    if let Some(rule) = rules
        .iter()
        .find(|rule| searchable.contains(&rule.pattern.to_lowercase()))
    {
        return (rule.category.clone(), 1.0);
    }
    let app_lower = app.to_lowercase();
    let category = if contains_any(&app_lower, &["zoom", "meet", "teams", "facetime"]) {
        "meetings"
    } else if contains_any(
        &app_lower,
        &["slack", "mail", "outlook", "whatsapp", "discord"],
    ) {
        "communication"
    } else if contains_any(
        &app_lower,
        &["code", "terminal", "xcode", "figma", "photoshop", "notion"],
    ) {
        "creation"
    } else if contains_any(&app_lower, &["kindle", "books", "coursera"]) {
        "learning"
    } else if contains_any(
        &app_lower,
        &["settings", "finder", "explorer", "excel", "sheets"],
    ) {
        "admin"
    } else {
        "unknown"
    };
    (
        category.into(),
        if category == "unknown" { 0.25 } else { 0.7 },
    )
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn ranges_overlap(
    a_start: DateTime<Utc>,
    a_end: DateTime<Utc>,
    b_start: DateTime<Utc>,
    b_end: DateTime<Utc>,
) -> bool {
    a_start < b_end && b_start < a_end
}

fn overlaps_strings(start: &str, end: &str, b_start: DateTime<Utc>, b_end: DateTime<Utc>) -> bool {
    let Ok(a_start) = crate::models::parse_utc(start) else {
        return false;
    };
    let Ok(a_end) = crate::models::parse_utc(end) else {
        return false;
    };
    ranges_overlap(a_start, a_end, b_start, b_end)
}

pub fn completed_interval(
    now: DateTime<Utc>,
    cadence_minutes: u16,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let seconds = i64::from(cadence_minutes) * 60;
    let end_seconds = now.timestamp().div_euclid(seconds) * seconds;
    let end = DateTime::from_timestamp(end_seconds, 0).unwrap_or(now);
    (end - Duration::seconds(seconds), end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooses_only_private_port_range() {
        let port = available_port().unwrap();
        assert!((PORT_START..=PORT_END).contains(&port));
    }

    #[test]
    fn aligns_completed_intervals() {
        let now = DateTime::parse_from_rfc3339("2026-08-21T12:23:30Z")
            .unwrap()
            .to_utc();
        let (start, end) = completed_interval(now, 15);
        assert_eq!(start.to_rfc3339(), "2026-08-21T12:00:00+00:00");
        assert_eq!(end.to_rfc3339(), "2026-08-21T12:15:00+00:00");
    }

    #[test]
    fn explicit_rules_win_over_defaults() {
        let rules = vec![AppRule {
            id: "one".into(),
            pattern: "notion".into(),
            category: "focus".into(),
            activity_label: None,
        }];
        assert_eq!(classify("Notion", None, &rules), ("focus".into(), 1.0));
    }
}
