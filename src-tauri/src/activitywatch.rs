use crate::models::{AppRule, CaptureIssueCode, CheckInV1, SettingsV1, TimelineBlockV1};
use aw_datastore::Datastore;
use aw_models::Event;
use chrono::{DateTime, Duration, Utc};
use std::fs::OpenOptions;
use std::io::Read;
use std::net::{Ipv4Addr, TcpListener};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration as StdDuration;
use uuid::Uuid;

const PORT_START: u16 = 5610;
const PORT_END: u16 = 5699;
const PROFILE_NAME: &str = "enso-time";
const LEGACY_APP_NAME: &str = "activitywatch";
const PROFILED_APP_NAME: &str = "activitywatch-enso-time";
const WATCHER_EXIT_GRACE: StdDuration = StdDuration::from_secs(2);
const WATCHER_REGISTRATION_TIMEOUT: StdDuration = StdDuration::from_secs(10);

struct ManagedWatcher {
    child: Child,
    stderr_path: PathBuf,
}

#[derive(Debug, Clone)]
struct WatcherProfilePaths {
    root: PathBuf,
    home: PathBuf,
    config: PathBuf,
    data: PathBuf,
    cache: PathBuf,
    state: PathBuf,
    roaming: PathBuf,
    local: PathBuf,
    diagnostics: PathBuf,
}

impl WatcherProfilePaths {
    fn new(root: PathBuf) -> Self {
        Self {
            home: root.join("home"),
            config: root.join("config"),
            data: root.join("data"),
            cache: root.join("cache"),
            state: root.join("state"),
            roaming: root.join("windows-roaming"),
            local: root.join("windows-local"),
            diagnostics: root.join("diagnostics"),
            root,
        }
    }

    fn prepare(&self) -> Result<(), String> {
        for directory in [
            &self.root,
            &self.home,
            &self.config,
            &self.data,
            &self.cache,
            &self.state,
            &self.roaming,
            &self.local,
            &self.diagnostics,
        ] {
            std::fs::create_dir_all(directory).map_err(|error| {
                format!(
                    "No se pudo crear el perfil privado del capturador en {}: {error}",
                    directory.display()
                )
            })?;
            set_private_directory_permissions(directory)?;
        }
        Ok(())
    }

    fn server_config_paths(&self) -> Vec<PathBuf> {
        [LEGACY_APP_NAME, PROFILED_APP_NAME]
            .into_iter()
            .flat_map(|app_name| {
                let mut paths = vec![self
                    .config
                    .join(app_name)
                    .join("aw-server-rust")
                    .join("config.toml")];
                #[cfg(target_os = "macos")]
                paths.push(
                    self.home
                        .join("Library")
                        .join("Application Support")
                        .join(app_name)
                        .join("aw-server-rust")
                        .join("config.toml"),
                );
                #[cfg(target_os = "windows")]
                {
                    paths.push(
                        self.local
                            .join(app_name)
                            .join("aw-server-rust")
                            .join("config.toml"),
                    );
                    paths.push(
                        self.roaming
                            .join(app_name)
                            .join("aw-server-rust")
                            .join("config.toml"),
                    );
                }
                paths
            })
            .collect()
    }

    fn configure(&self, command: &mut Command) {
        // v0.14.0b3 predates AW_PROFILE. The platform roots isolate that release,
        // while AW_PROFILE keeps the same contract for newer ActivityWatch builds.
        command
            .env("AW_PROFILE", PROFILE_NAME)
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", &self.config)
            .env("XDG_DATA_HOME", &self.data)
            .env("XDG_CACHE_HOME", &self.cache)
            .env("XDG_STATE_HOME", &self.state)
            .env("APPDATA", &self.roaming)
            .env("LOCALAPPDATA", &self.local)
            .env("USERPROFILE", &self.home);
    }
}

pub struct ActivityWatchRuntime {
    datastore: Datastore,
    port: AtomicU16,
    server_ready: Arc<AtomicBool>,
    server_failed: Arc<Mutex<Option<String>>>,
    server_shutdown: Arc<Mutex<Option<rocket::Shutdown>>>,
    server_thread: Mutex<Option<JoinHandle<()>>>,
    capture_issue: Arc<Mutex<Option<CaptureIssueCode>>>,
    window_watcher: Arc<Mutex<Option<ManagedWatcher>>>,
    afk_watcher: Arc<Mutex<Option<ManagedWatcher>>>,
    resource_dir: PathBuf,
    profile: WatcherProfilePaths,
}

impl ActivityWatchRuntime {
    pub fn start(data_dir: &Path, resource_dir: PathBuf) -> Result<Self, String> {
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
        let profile = WatcherProfilePaths::new(data_dir.join("watcher-profile"));
        profile.prepare()?;
        write_watcher_server_config(&profile, port, &api_key)?;
        let datastore = Datastore::new(database, false);
        let server_datastore = datastore.clone();
        let server_ready = Arc::new(AtomicBool::new(false));
        let server_failed = Arc::new(Mutex::new(None));
        let server_shutdown = Arc::new(Mutex::new(None));
        let capture_issue = Arc::new(Mutex::new(None));
        let window_watcher = Arc::new(Mutex::new(None));
        let afk_watcher = Arc::new(Mutex::new(None));
        let failed = Arc::clone(&server_failed);
        let shutdown = Arc::clone(&server_shutdown);
        let issue = Arc::clone(&capture_issue);
        let ready = Arc::clone(&server_ready);
        let window_on_server_exit = Arc::clone(&window_watcher);
        let afk_on_server_exit = Arc::clone(&afk_watcher);
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        // Rocket owns its async runtime. It is isolated from Tauri so a local capture
        // failure cannot take down the UI thread.
        let server_thread = std::thread::Builder::new()
            .name("enso-aw-server".into())
            .spawn(move || {
                let result = run_server(
                    server_datastore,
                    port,
                    api_key,
                    ready.clone(),
                    shutdown,
                    startup_sender,
                );
                ready.store(false, Ordering::Release);
                if let Err(error) = result {
                    if let Ok(mut slot) = failed.lock() {
                        *slot = Some(error);
                    }
                    if let Ok(mut slot) = issue.lock() {
                        *slot = Some(CaptureIssueCode::ServerUnavailable);
                    }
                    let _ = terminate_slot(&window_on_server_exit);
                    let _ = terminate_slot(&afk_on_server_exit);
                }
            })
            .map_err(|error| format!("No se pudo iniciar el servicio local: {error}"))?;
        match startup_receiver.recv_timeout(StdDuration::from_secs(5)) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                let _ = server_thread.join();
                return Err(error);
            }
            Err(error) => {
                if let Ok(slot) = server_shutdown.lock() {
                    if let Some(shutdown) = slot.as_ref() {
                        shutdown.clone().notify();
                    }
                }
                let _ = server_thread.join();
                return Err(format!(
                    "El servicio local no completó su inicio de forma verificable: {error}"
                ));
            }
        }
        Ok(Self {
            datastore,
            port: AtomicU16::new(port),
            server_ready,
            server_failed,
            server_shutdown,
            server_thread: Mutex::new(Some(server_thread)),
            capture_issue,
            window_watcher,
            afk_watcher,
            resource_dir,
            profile,
        })
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

    pub fn capture_issue(&self) -> Option<CaptureIssueCode> {
        self.capture_issue.lock().ok().and_then(|value| *value)
    }

    pub fn start_watchers(&self, capture_titles: bool) -> Result<(), String> {
        let port = self.ready_port().map_err(|error| {
            self.set_capture_issue(CaptureIssueCode::ServerUnavailable);
            error
        })?;
        if let Err(error) = self.start_watcher(WatcherKind::Window, port, capture_titles) {
            self.set_capture_issue(error.code);
            let _ = self.stop_watchers();
            return Err(error.message);
        }
        if let Err(error) = self.start_watcher(WatcherKind::Afk, port, capture_titles) {
            self.set_capture_issue(error.code);
            let _ = self.stop_watchers();
            return Err(error.message);
        }
        self.clear_capture_issue();
        Ok(())
    }

    pub fn restart_window_watcher(&self, capture_titles: bool) -> Result<(), String> {
        // Never stop a healthy watcher until Rocket has confirmed that the private
        // loopback endpoint is ours. This also closes the bind/start TOCTOU window.
        let port = self.ready_port().map_err(|error| {
            self.set_capture_issue(CaptureIssueCode::ServerUnavailable);
            error
        })?;
        terminate_slot(&self.window_watcher)?;
        if let Err(error) = self.start_watcher(WatcherKind::Window, port, capture_titles) {
            self.set_capture_issue(error.code);
            let _ = self.stop_watchers();
            return Err(error.message);
        }
        self.clear_capture_issue();
        Ok(())
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
        if self.server_error().is_some() {
            self.set_capture_issue(CaptureIssueCode::ServerUnavailable);
            let _ = self.stop_watchers();
        }
        let window = inspect_watcher(
            &self.window_watcher,
            self.binary_path(WatcherKind::Window).exists(),
            WatcherKind::Window,
        );
        let afk = inspect_watcher(
            &self.afk_watcher,
            self.binary_path(WatcherKind::Afk).exists(),
            WatcherKind::Afk,
        );
        if let Some(issue) = window.issue.or(afk.issue) {
            self.set_capture_issue(issue);
            let _ = self.stop_watchers();
        }
        (window.status, afk.status)
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
        // Every platform directory used by either the pinned legacy client or a
        // profile-aware future client lives below this one Ensō-owned root.
        if self.profile.root.exists() {
            delete_private_queues(&self.profile.root)?;
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
    ) -> Result<(), WatcherStartError> {
        let slot = match kind {
            WatcherKind::Window => &self.window_watcher,
            WatcherKind::Afk => &self.afk_watcher,
        };
        if matches!(
            inspect_watcher(slot, self.binary_path(kind).exists(), kind)
                .status
                .as_str(),
            "running"
        ) {
            return Ok(());
        }
        let binary = self.binary_path(kind);
        if !binary.is_file() {
            return Err(WatcherStartError::unavailable(
                kind,
                format!(
                    "No se encontró el capturador {} en {}",
                    kind.label(),
                    binary.display()
                ),
            ));
        }
        let bucket_was_present = self
            .has_bucket_type(kind.bucket_type())
            .map_err(|message| WatcherStartError::unavailable(kind, message))?;
        let stderr_path = self
            .profile
            .diagnostics
            .join(format!("{}.stderr.log", kind.binary_name()));
        let stderr = private_log_file(&stderr_path)
            .map_err(|message| WatcherStartError::unavailable(kind, message))?;
        let mut command = Command::new(&binary);
        command
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr));
        self.profile.configure(&mut command);
        if matches!(kind, WatcherKind::Window) && !capture_titles {
            command.arg("--exclude-title");
        }
        let child = command.spawn().map_err(|error| {
            WatcherStartError::unavailable(
                kind,
                format!("No se pudo iniciar el capturador {}: {error}", kind.label()),
            )
        })?;
        *slot.lock().map_err(|_| {
            WatcherStartError::unavailable(
                kind,
                "No se pudo actualizar el estado del capturador".into(),
            )
        })? = Some(ManagedWatcher {
            child,
            stderr_path: stderr_path.clone(),
        });

        let deadline = std::time::Instant::now()
            + if bucket_was_present {
                WATCHER_EXIT_GRACE
            } else {
                WATCHER_REGISTRATION_TIMEOUT
            };
        loop {
            if let Some(status) = watcher_exit_status(slot)
                .map_err(|message| WatcherStartError::unavailable(kind, message))?
            {
                let diagnostic = read_diagnostic(&stderr_path);
                return Err(WatcherStartError::from_exit(kind, status, &diagnostic));
            }
            if !bucket_was_present && self.has_bucket_type(kind.bucket_type()).unwrap_or(false) {
                return Ok(());
            } else if bucket_was_present && std::time::Instant::now() >= deadline {
                return Ok(());
            } else if std::time::Instant::now() >= deadline {
                let _ = terminate_slot(slot);
                let diagnostic = read_diagnostic(&stderr_path);
                return Err(WatcherStartError::unavailable(
                    kind,
                    format!(
                        "El capturador {} no se registró en el servicio privado a tiempo{}",
                        kind.label(),
                        diagnostic_suffix(&diagnostic)
                    ),
                ));
            }
            std::thread::sleep(StdDuration::from_millis(50));
        }
    }

    fn has_bucket_type(&self, bucket_type: &str) -> Result<bool, String> {
        let buckets = safe_datastore(|| self.datastore.get_buckets())?;
        Ok(buckets.values().any(|bucket| bucket._type == bucket_type))
    }

    fn set_capture_issue(&self, issue: CaptureIssueCode) {
        if let Ok(mut slot) = self.capture_issue.lock() {
            *slot = Some(issue);
        }
    }

    fn clear_capture_issue(&self) {
        if let Ok(mut slot) = self.capture_issue.lock() {
            *slot = None;
        }
    }

    fn shutdown_server(&self) {
        if let Ok(slot) = self.server_shutdown.lock() {
            if let Some(shutdown) = slot.as_ref() {
                shutdown.clone().notify();
            }
        }
        if let Ok(mut slot) = self.server_thread.lock() {
            if let Some(thread) = slot.take() {
                let _ = thread.join();
            }
        }
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
        self.shutdown_server();
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

    fn bucket_type(self) -> &'static str {
        match self {
            Self::Window => "currentwindow",
            Self::Afk => "afkstatus",
        }
    }

    fn unavailable_code(self) -> CaptureIssueCode {
        match self {
            Self::Window => CaptureIssueCode::WindowWatcherUnavailable,
            Self::Afk => CaptureIssueCode::AfkWatcherUnavailable,
        }
    }
}

struct WatcherStartError {
    code: CaptureIssueCode,
    message: String,
}

impl WatcherStartError {
    fn unavailable(kind: WatcherKind, message: String) -> Self {
        Self {
            code: kind.unavailable_code(),
            message,
        }
    }

    fn from_exit(kind: WatcherKind, status: ExitStatus, diagnostic: &str) -> Self {
        let conflict = diagnostic.to_ascii_lowercase().contains("another instance")
            || diagnostic.to_ascii_lowercase().contains("already running")
            || diagnostic.to_ascii_lowercase().contains("lock conflict");
        Self {
            code: if conflict {
                CaptureIssueCode::WatcherConflict
            } else {
                kind.unavailable_code()
            },
            message: format!(
                "El capturador {} terminó durante el inicio ({status}){}",
                kind.label(),
                diagnostic_suffix(diagnostic)
            ),
        }
    }
}

fn run_server(
    datastore: Datastore,
    port: u16,
    api_key: String,
    ready: Arc<AtomicBool>,
    shutdown_slot: Arc<Mutex<Option<rocket::Shutdown>>>,
    startup_sender: mpsc::SyncSender<Result<(), String>>,
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
        let rocket = match aw_server::endpoints::build_rocket(state, config)
            .attach(ready_fairing)
            .ignite()
            .await
        {
            Ok(rocket) => rocket,
            Err(error) => {
                let message = format!("No se pudo preparar el servicio ActivityWatch: {error}");
                let _ = startup_sender.send(Err(message.clone()));
                return Err(message);
            }
        };
        let shutdown = rocket.shutdown();
        *shutdown_slot
            .lock()
            .map_err(|_| "No se pudo preparar el apagado del servicio local")? = Some(shutdown);
        let _ = startup_sender.send(Ok(()));
        rocket
            .launch()
            .await
            .map(|_| ())
            .map_err(|error| format!("El servicio ActivityWatch se detuvo: {error}"))
    })
}

fn write_watcher_server_config(
    profile: &WatcherProfilePaths,
    port: u16,
    api_key: &str,
) -> Result<(), String> {
    let contents =
        format!("address = \"127.0.0.1\"\nport = {port}\n\n[auth]\napi_key = \"{api_key}\"\n");
    for path in profile.server_config_paths() {
        let directory = path
            .parent()
            .ok_or_else(|| "La ruta de configuración privada no es válida".to_string())?;
        std::fs::create_dir_all(directory).map_err(|error| {
            format!("No se pudo crear la configuración privada del capturador: {error}")
        })?;
        set_private_directory_permissions(directory)?;
        std::fs::write(&path, &contents)
            .map_err(|error| format!("No se pudo proteger el servicio local: {error}"))?;
        set_private_file_permissions(&path)?;
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

fn set_private_directory_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(
            |error| {
                format!(
                    "No se pudo restringir el directorio privado {}: {error}",
                    path.display()
                )
            },
        )?;
    }
    Ok(())
}

fn set_private_file_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(
            |error| {
                format!(
                    "No se pudo restringir el archivo privado {}: {error}",
                    path.display()
                )
            },
        )?;
    }
    Ok(())
}

fn private_log_file(path: &Path) -> Result<std::fs::File, String> {
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path).map_err(|error| {
        format!(
            "No se pudo abrir el diagnóstico privado {}: {error}",
            path.display()
        )
    })?;
    set_private_file_permissions(path)?;
    Ok(file)
}

fn read_diagnostic(path: &Path) -> String {
    let Ok(file) = std::fs::File::open(path) else {
        return String::new();
    };
    let mut output = String::new();
    let _ = file.take(8 * 1024).read_to_string(&mut output);
    output.trim().to_string()
}

fn diagnostic_suffix(diagnostic: &str) -> String {
    let line = diagnostic.lines().last().unwrap_or_default().trim();
    if line.is_empty() {
        String::new()
    } else {
        format!(": {line}")
    }
}

fn walk_queue_directories(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut queues = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory).map_err(|error| {
            format!(
                "No se pudo revisar el perfil privado {}: {error}",
                directory.display()
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if !file_type.is_dir() {
                continue;
            }
            let path = entry.path();
            if entry.file_name() == "queued" {
                queues.push(path);
            } else {
                pending.push(path);
            }
        }
    }
    Ok(queues)
}

fn delete_private_queues(root: &Path) -> Result<(), String> {
    for entry in walk_queue_directories(root)? {
        std::fs::remove_dir_all(&entry).map_err(|error| {
            format!(
                "No se pudo borrar la cola privada {}: {error}",
                entry.display()
            )
        })?;
    }
    Ok(())
}

fn safe_datastore<T, F>(operation: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, aw_datastore::DatastoreError>,
{
    catch_unwind(AssertUnwindSafe(operation))
        .map_err(|_| "El almacén de actividad dejó de responder".to_string())?
        .map_err(|error| format!("Error del almacén de actividad: {error:?}"))
}

struct WatcherInspection {
    status: String,
    issue: Option<CaptureIssueCode>,
}

fn inspect_watcher(
    slot: &Mutex<Option<ManagedWatcher>>,
    binary_exists: bool,
    kind: WatcherKind,
) -> WatcherInspection {
    let Ok(mut guard) = slot.lock() else {
        return WatcherInspection {
            status: "error".into(),
            issue: Some(kind.unavailable_code()),
        };
    };
    let Some(watcher) = guard.as_mut() else {
        return WatcherInspection {
            status: if binary_exists { "stopped" } else { "missing" }.into(),
            issue: None,
        };
    };
    match watcher.child.try_wait() {
        Ok(None) => WatcherInspection {
            status: "running".into(),
            issue: None,
        },
        Ok(Some(_)) => {
            let diagnostic = read_diagnostic(&watcher.stderr_path).to_ascii_lowercase();
            let conflict = diagnostic.contains("another instance")
                || diagnostic.contains("already running")
                || diagnostic.contains("lock conflict");
            *guard = None;
            WatcherInspection {
                status: "stopped".into(),
                issue: Some(if conflict {
                    CaptureIssueCode::WatcherConflict
                } else {
                    kind.unavailable_code()
                }),
            }
        }
        Err(_) => WatcherInspection {
            status: "error".into(),
            issue: Some(kind.unavailable_code()),
        },
    }
}

fn watcher_exit_status(slot: &Mutex<Option<ManagedWatcher>>) -> Result<Option<ExitStatus>, String> {
    let mut guard = slot
        .lock()
        .map_err(|_| "No se pudo consultar el capturador".to_string())?;
    let Some(watcher) = guard.as_mut() else {
        return Ok(None);
    };
    watcher.child.try_wait().map_err(|error| error.to_string())
}

fn terminate_slot(slot: &Mutex<Option<ManagedWatcher>>) -> Result<(), String> {
    let mut guard = slot
        .lock()
        .map_err(|_| "No se pudo detener el capturador")?;
    let Some(mut watcher) = guard.take() else {
        return Ok(());
    };
    let child = &mut watcher.child;
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
    use aw_models::{Bucket, BucketMetadata};
    use serde_json::Map;
    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};
    use std::net::TcpListener;
    use tempfile::TempDir;

    #[test]
    fn chooses_only_private_port_range() {
        let official = TcpListener::bind((Ipv4Addr::LOCALHOST, 5600)).unwrap();
        let port = available_port().unwrap();
        assert!((PORT_START..=PORT_END).contains(&port));
        assert_ne!(official.local_addr().unwrap().port(), port);
    }

    #[test]
    fn watcher_profile_environment_is_entirely_below_private_root() {
        let temp = TempDir::new().unwrap();
        let profile = WatcherProfilePaths::new(temp.path().join("profile"));
        profile.prepare().unwrap();
        let mut command = Command::new("ignored");
        profile.configure(&mut command);
        let env: HashMap<OsString, Option<OsString>> = command
            .get_envs()
            .map(|(key, value)| (key.to_owned(), value.map(OsString::from)))
            .collect();
        assert_eq!(
            env.get(&OsString::from("AW_PROFILE"))
                .and_then(Option::as_ref)
                .map(OsString::as_os_str),
            Some(OsStr::new(PROFILE_NAME))
        );
        for key in [
            "HOME",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "XDG_STATE_HOME",
            "APPDATA",
            "LOCALAPPDATA",
            "USERPROFILE",
        ] {
            let path = PathBuf::from(
                env.get(&OsString::from(key))
                    .and_then(Option::as_ref)
                    .unwrap(),
            );
            assert!(path.starts_with(&profile.root), "{key}: {}", path.display());
        }
    }

    #[test]
    fn server_configs_cover_legacy_and_profiled_clients_without_leaking() {
        let temp = TempDir::new().unwrap();
        let profile = WatcherProfilePaths::new(temp.path().join("profile"));
        profile.prepare().unwrap();
        write_watcher_server_config(&profile, 5655, "private-key").unwrap();
        let paths = profile.server_config_paths();
        assert!(paths.len() >= 2);
        assert!(paths.iter().any(|path| path.ends_with(
            Path::new(LEGACY_APP_NAME)
                .join("aw-server-rust")
                .join("config.toml")
        )));
        assert!(paths.iter().any(|path| path.ends_with(
            Path::new(PROFILED_APP_NAME)
                .join("aw-server-rust")
                .join("config.toml")
        )));
        for path in paths {
            assert!(path.starts_with(&profile.root));
            let contents = std::fs::read_to_string(path).unwrap();
            assert!(contents.contains("port = 5655"));
            assert!(contents.contains("api_key = \"private-key\""));
        }
    }

    #[test]
    fn queue_cleanup_only_removes_private_queued_directories() {
        let temp = TempDir::new().unwrap();
        let profile = WatcherProfilePaths::new(temp.path().join("profile"));
        profile.prepare().unwrap();
        let legacy_queue = profile
            .home
            .join("Library/Application Support/activitywatch/aw-client/queued");
        let profiled_queue = profile
            .data
            .join(PROFILED_APP_NAME)
            .join("aw-client/queued");
        for queue in [&legacy_queue, &profiled_queue] {
            std::fs::create_dir_all(queue).unwrap();
            std::fs::write(queue.join("event.persistqueue"), "preserve until delete").unwrap();
        }
        let unrelated = profile.root.join("keep/config.toml");
        std::fs::create_dir_all(unrelated.parent().unwrap()).unwrap();
        std::fs::write(&unrelated, "keep").unwrap();

        delete_private_queues(&profile.root).unwrap();

        assert!(!legacy_queue.exists());
        assert!(!profiled_queue.exists());
        assert_eq!(std::fs::read_to_string(unrelated).unwrap(), "keep");
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

    #[cfg(unix)]
    #[test]
    fn watcher_lifecycle_is_enso_owned_and_server_port_is_released() {
        let temp = TempDir::new().unwrap();
        let resources = fake_watcher_resources(
            temp.path(),
            "echo \"args:$*\" >&2; echo \"profile:$AW_PROFILE home:$HOME\" >&2; exec sleep 30",
        );
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        let port = wait_until_ready(&runtime);
        seed_watcher_buckets(&runtime);

        runtime.start_watchers(false).unwrap();
        assert_eq!(
            runtime.watcher_statuses(),
            ("running".into(), "running".into())
        );
        assert_eq!(runtime.capture_issue(), None);
        for kind in [WatcherKind::Window, WatcherKind::Afk] {
            let diagnostic = read_diagnostic(
                &runtime
                    .profile
                    .diagnostics
                    .join(format!("{}.stderr.log", kind.binary_name())),
            );
            assert!(diagnostic.contains(&format!("--port {port}")));
            assert!(diagnostic.contains("profile:enso-time"));
            assert!(diagnostic.contains(&runtime.profile.home.to_string_lossy().to_string()));
        }
        runtime.stop_watchers().unwrap();
        assert_eq!(
            runtime.watcher_statuses(),
            ("stopped".into(), "stopped".into())
        );
        drop(runtime);

        TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .expect("the private Rocket listener must stop with the Ensō runtime");
    }

    #[cfg(unix)]
    #[test]
    fn early_single_instance_exit_is_reported_as_conflict_and_can_retry() {
        let temp = TempDir::new().unwrap();
        let resources = fake_watcher_resources(
            temp.path(),
            "echo 'Another instance is already running, quitting.' >&2; exit 255",
        );
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        wait_until_ready(&runtime);
        seed_watcher_buckets(&runtime);

        let error = runtime.start_watchers(false).unwrap_err();
        assert!(error.contains("terminó durante el inicio"));
        assert_eq!(
            runtime.capture_issue(),
            Some(CaptureIssueCode::WatcherConflict)
        );
        assert_eq!(
            runtime.watcher_statuses(),
            ("stopped".into(), "stopped".into())
        );
    }

    #[cfg(unix)]
    #[test]
    fn missing_afk_watcher_stops_only_enso_capture_and_reports_exact_issue() {
        use std::os::unix::fs::PermissionsExt;
        let temp = TempDir::new().unwrap();
        let resources = temp.path().join("resources");
        let directory = resources.join("activitywatch").join("aw-watcher-window");
        std::fs::create_dir_all(&directory).unwrap();
        let binary = directory.join("aw-watcher-window");
        std::fs::write(&binary, "#!/bin/sh\nexec sleep 30\n").unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        wait_until_ready(&runtime);
        seed_watcher_buckets(&runtime);

        assert!(runtime.start_watchers(false).is_err());
        assert_eq!(
            runtime.capture_issue(),
            Some(CaptureIssueCode::AfkWatcherUnavailable)
        );
        assert_eq!(
            runtime.watcher_statuses(),
            ("stopped".into(), "missing".into())
        );
    }

    #[cfg(unix)]
    fn fake_watcher_resources(root: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let resources = root.join("resources");
        for name in ["aw-watcher-window", "aw-watcher-afk"] {
            let directory = resources.join("activitywatch").join(name);
            std::fs::create_dir_all(&directory).unwrap();
            let binary = directory.join(name);
            std::fs::write(&binary, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        resources
    }

    fn wait_until_ready(runtime: &ActivityWatchRuntime) -> u16 {
        for _ in 0..100 {
            if let Some(port) = runtime.port_if_ready() {
                return port;
            }
            if let Some(error) = runtime.server_error() {
                panic!("private server failed: {error}");
            }
            std::thread::sleep(StdDuration::from_millis(25));
        }
        panic!("private server did not become ready");
    }

    fn seed_watcher_buckets(runtime: &ActivityWatchRuntime) {
        for (id, bucket_type) in [("test-window", "currentwindow"), ("test-afk", "afkstatus")] {
            runtime
                .datastore
                .create_bucket(&Bucket {
                    bid: None,
                    id: id.into(),
                    _type: bucket_type.into(),
                    client: "enso-test".into(),
                    hostname: "localhost".into(),
                    created: None,
                    data: Map::new(),
                    metadata: BucketMetadata::default(),
                    events: None,
                    last_updated: None,
                })
                .unwrap();
        }
    }
}
