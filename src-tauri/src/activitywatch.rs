use crate::models::{
    AppRule, CaptureIssueCode, CheckInV1, ClassificationOverrideV1, ClassificationRuleV1,
    SettingsV1, TimelineBlockV1,
};
use aw_datastore::Datastore;
use aw_models::Event;
use chrono::{DateTime, Duration, Utc};
use rusqlite::Connection;
use std::fs::OpenOptions;
use std::io::Read;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
#[cfg(any(not(target_os = "macos"), test))]
use std::process::Stdio;
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration as StdDuration, Instant};
use uuid::Uuid;

const PORT_START: u16 = 5610;
const PORT_END: u16 = 5699;
const PROFILE_NAME: &str = "enso-time";
const LEGACY_APP_NAME: &str = "activitywatch";
const PROFILED_APP_NAME: &str = "activitywatch-enso-time";
const WATCHER_REGISTRATION_TIMEOUT: StdDuration = StdDuration::from_secs(20);
const WATCHER_STALE_TIMEOUT: StdDuration = StdDuration::from_secs(45);
#[cfg(all(target_os = "macos", not(test)))]
const WINDOW_HEARTBEAT_PULSETIME_SECONDS: f64 = 5.0;
#[cfg(all(target_os = "macos", not(test)))]
const AFK_HEARTBEAT_PULSETIME_SECONDS: f64 = 15.0;
#[cfg(all(target_os = "macos", not(test)))]
const AFK_IDLE_THRESHOLD_SECONDS: f64 = 180.0;
const CAPTURE_START_CANCELLED: &str = "La operación de captura fue cancelada";
const WINDOW_PERMISSION_REQUIRED: &str = "El permiso de Accesibilidad no está disponible";
#[cfg(target_os = "macos")]
const WINDOW_HELPER_BINARY: &str = "enso-aw-watcher-window-macos";
#[cfg(all(target_os = "macos", test))]
const AFK_HELPER_BINARY: &str = "enso-aw-watcher-afk-macos";
#[cfg(target_os = "macos")]
const WINDOW_PERMISSION_EXIT: i32 = 77;
#[cfg(target_os = "macos")]
const ACTIVITYWATCH_API_KEY_ENV: &str = "ENSO_ACTIVITYWATCH_API_KEY";

struct ManagedWatcher {
    child: Child,
    stderr_path: PathBuf,
}

#[cfg(all(target_os = "macos", not(test)))]
struct ManagedNativeWorker {
    stop: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowCaptureMode {
    Stopped,
    Full,
    AppOnlyPreference,
    AppOnlyPermission,
}

#[derive(Debug)]
struct WatcherRetryState {
    failures: u32,
    next_retry: Option<Instant>,
}

impl WatcherRetryState {
    fn new() -> Self {
        Self {
            failures: 0,
            next_retry: None,
        }
    }

    fn due(&self) -> bool {
        self.next_retry
            .is_none_or(|deadline| Instant::now() >= deadline)
    }

    fn success(&mut self) {
        self.failures = 0;
        self.next_retry = None;
    }

    fn failure(&mut self) {
        self.failures = self.failures.saturating_add(1);
        let delay = match self.failures {
            1 => 30,
            2 => 60,
            3 => 120,
            _ => 300,
        };
        self.next_retry = Some(Instant::now() + StdDuration::from_secs(delay));
    }

    #[cfg(test)]
    fn retry_delay(&self) -> Option<StdDuration> {
        self.next_retry
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }
}

struct RuntimeCancellation {
    cancelled: AtomicBool,
    wait_lock: Mutex<()>,
    wait_signal: Condvar,
}

impl RuntimeCancellation {
    fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            wait_lock: Mutex::new(()),
            wait_signal: Condvar::new(),
        }
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.wait_signal.notify_all();
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn wait_timeout(&self, timeout: StdDuration) -> bool {
        if self.is_cancelled() {
            return true;
        }
        let Ok(guard) = self.wait_lock.lock() else {
            return true;
        };
        let _ = self
            .wait_signal
            .wait_timeout_while(guard, timeout, |_| !self.is_cancelled());
        self.is_cancelled()
    }
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
    datastore: Mutex<Option<Datastore>>,
    database_path: PathBuf,
    port: Arc<AtomicU16>,
    server_ready: Arc<AtomicBool>,
    server_failed: Arc<Mutex<Option<String>>>,
    server_shutdown: Arc<Mutex<Option<rocket::Shutdown>>>,
    server_thread: Mutex<Option<JoinHandle<()>>>,
    capture_issue: Arc<Mutex<Option<CaptureIssueCode>>>,
    capture_titles: Arc<AtomicBool>,
    window_capture_mode: Arc<Mutex<WindowCaptureMode>>,
    watcher_retry: Mutex<WatcherRetryState>,
    capture_generation: AtomicU64,
    capture_suspended: AtomicBool,
    shutdown_started: AtomicBool,
    cancellation: Arc<RuntimeCancellation>,
    watcher_lifecycle: Mutex<()>,
    #[cfg(test)]
    watcher_lifecycle_lock_attempt: Mutex<Option<mpsc::Sender<()>>>,
    window_health: Mutex<Option<WatcherHealth>>,
    afk_health: Mutex<Option<WatcherHealth>>,
    window_watcher: Arc<Mutex<Option<ManagedWatcher>>>,
    #[cfg(all(target_os = "macos", not(test)))]
    window_worker: Mutex<Option<ManagedNativeWorker>>,
    #[cfg(all(target_os = "macos", not(test)))]
    afk_worker: Mutex<Option<ManagedNativeWorker>>,
    afk_watcher: Arc<Mutex<Option<ManagedWatcher>>>,
    resource_dir: PathBuf,
    profile: WatcherProfilePaths,
    api_key: String,
    connector_gateway: Option<Arc<crate::connectors::ConnectorGateway>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BucketSnapshot {
    client: String,
    hostname: String,
    bucket_type: String,
    created: Option<DateTime<Utc>>,
    last_updated: Option<DateTime<Utc>>,
    event_end: Option<DateTime<Utc>>,
    event_count: i64,
}

#[derive(Debug, Clone)]
struct WatcherBaseline {
    bucket_id: String,
    client: &'static str,
    hostname: String,
    bucket_type: &'static str,
    snapshot: Option<BucketSnapshot>,
}

#[derive(Debug, Clone)]
struct WatcherHealth {
    baseline: WatcherBaseline,
    last_snapshot: BucketSnapshot,
    last_progress: Instant,
}

impl ActivityWatchRuntime {
    pub fn start(data_dir: &Path, resource_dir: PathBuf) -> Result<Self, String> {
        Self::start_inner(data_dir, resource_dir, None)
    }

    pub fn start_with_gateway(
        data_dir: &Path,
        resource_dir: PathBuf,
        connector_gateway: Arc<crate::connectors::ConnectorGateway>,
    ) -> Result<Self, String> {
        Self::start_inner(data_dir, resource_dir, Some(connector_gateway))
    }

    fn start_inner(
        data_dir: &Path,
        resource_dir: PathBuf,
        connector_gateway: Option<Arc<crate::connectors::ConnectorGateway>>,
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
        prepare_activity_database(&database)?;
        let database_string = database
            .to_str()
            .ok_or_else(|| "La ruta del almacén de actividad no es válida".to_string())?
            .to_string();
        let api_key = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let profile = WatcherProfilePaths::new(data_dir.join("watcher-profile"));
        profile.prepare()?;
        let datastore = Datastore::new(database_string, false);
        let port = Arc::new(AtomicU16::new(0));
        let server_ready = Arc::new(AtomicBool::new(false));
        let server_failed = Arc::new(Mutex::new(None));
        let server_shutdown = Arc::new(Mutex::new(None));
        let capture_issue = Arc::new(Mutex::new(None));
        let window_watcher = Arc::new(Mutex::new(None));
        let afk_watcher = Arc::new(Mutex::new(None));
        let runtime = Self {
            datastore: Mutex::new(Some(datastore)),
            database_path: database,
            port,
            server_ready,
            server_failed,
            server_shutdown,
            server_thread: Mutex::new(None),
            capture_issue,
            capture_titles: Arc::new(AtomicBool::new(false)),
            window_capture_mode: Arc::new(Mutex::new(WindowCaptureMode::Stopped)),
            watcher_retry: Mutex::new(WatcherRetryState::new()),
            capture_generation: AtomicU64::new(0),
            capture_suspended: AtomicBool::new(false),
            shutdown_started: AtomicBool::new(false),
            cancellation: Arc::new(RuntimeCancellation::new()),
            watcher_lifecycle: Mutex::new(()),
            #[cfg(test)]
            watcher_lifecycle_lock_attempt: Mutex::new(None),
            window_health: Mutex::new(None),
            afk_health: Mutex::new(None),
            window_watcher,
            #[cfg(all(target_os = "macos", not(test)))]
            window_worker: Mutex::new(None),
            #[cfg(all(target_os = "macos", not(test)))]
            afk_worker: Mutex::new(None),
            afk_watcher,
            resource_dir,
            profile,
            api_key,
            connector_gateway,
        };
        runtime.launch_server()?;
        Ok(runtime)
    }

    fn datastore(&self) -> Result<Datastore, String> {
        self.datastore
            .lock()
            .map_err(|_| "No se pudo acceder al almacén de actividad".to_string())?
            .clone()
            .ok_or_else(|| "El almacén de actividad está en mantenimiento".to_string())
    }

    fn launch_server(&self) -> Result<(), String> {
        if self.cancellation.is_cancelled() {
            return Err("El runtime de captura ya se cerró".into());
        }
        let server_datastore = self.datastore()?;
        self.port.store(0, Ordering::Release);
        self.server_ready.store(false, Ordering::Release);
        if let Ok(mut failed) = self.server_failed.lock() {
            *failed = None;
        }
        let failed = Arc::clone(&self.server_failed);
        let shutdown = Arc::clone(&self.server_shutdown);
        let issue = Arc::clone(&self.capture_issue);
        let ready = Arc::clone(&self.server_ready);
        let window_on_server_exit = Arc::clone(&self.window_watcher);
        let afk_on_server_exit = Arc::clone(&self.afk_watcher);
        let server_port = Arc::clone(&self.port);
        let server_profile = self.profile.clone();
        let server_api_key = self.api_key.clone();
        let connector_gateway = self.connector_gateway.clone();
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        // Rocket owns its async runtime. It is isolated from Tauri so a local capture
        // failure cannot take down the UI thread.
        let server_thread = std::thread::Builder::new()
            .name("enso-aw-server".into())
            .spawn(move || {
                let result = run_server(
                    server_datastore,
                    server_port,
                    server_api_key,
                    server_profile,
                    connector_gateway,
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
        match startup_receiver.recv_timeout(StdDuration::from_secs(10)) {
            Ok(Ok(())) => {
                *self
                    .server_thread
                    .lock()
                    .map_err(|_| "No se pudo registrar el servicio local")? = Some(server_thread);
                Ok(())
            }
            Ok(Err(error)) => {
                let _ = server_thread.join();
                Err(error)
            }
            Err(error) => {
                if let Ok(slot) = self.server_shutdown.lock() {
                    if let Some(shutdown) = slot.as_ref() {
                        shutdown.clone().notify();
                    }
                }
                let _ = server_thread.join();
                Err(format!(
                    "El servicio local no completó su inicio de forma verificable: {error}"
                ))
            }
        }
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
        let fatal = self.capture_issue.lock().ok().and_then(|value| *value);
        fatal.or_else(|| {
            matches!(
                self.window_capture_mode.lock().ok().as_deref(),
                Some(WindowCaptureMode::AppOnlyPermission)
            )
            .then_some(CaptureIssueCode::AccessibilityPermissionRequired)
        })
    }

    pub fn start_watchers(&self, capture_titles: bool) -> Result<(), String> {
        #[cfg(test)]
        self.notify_watcher_lifecycle_lock_attempt();
        let _lifecycle = self
            .watcher_lifecycle
            .lock()
            .map_err(|_| "No se pudo coordinar el capturador".to_string())?;
        if self.cancellation.is_cancelled() || self.capture_suspended.load(Ordering::Acquire) {
            return Err(CAPTURE_START_CANCELLED.into());
        }
        let result = self.start_watchers_with_timeout(capture_titles, WATCHER_REGISTRATION_TIMEOUT);
        self.record_start_result(&result);
        result
    }

    pub fn start_watchers_if_due(&self, capture_titles: bool) -> Result<bool, String> {
        if !self
            .watcher_retry
            .lock()
            .map_err(|_| "No se pudo consultar el reintento del capturador".to_string())?
            .due()
        {
            return Ok(false);
        }
        self.retry_watchers(capture_titles).map(|_| true)
    }

    fn record_start_result(&self, result: &Result<(), String>) {
        let Ok(mut retry) = self.watcher_retry.lock() else {
            return;
        };
        match result {
            Ok(()) => retry.success(),
            Err(error) if error == CAPTURE_START_CANCELLED => {}
            Err(_) => retry.failure(),
        }
    }

    #[cfg(test)]
    fn notify_watcher_lifecycle_lock_attempt(&self) {
        let sender = self
            .watcher_lifecycle_lock_attempt
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());
        if let Some(sender) = sender {
            let _ = sender.send(());
        }
    }

    #[cfg(test)]
    fn signal_watcher_lifecycle_lock_attempt(&self, sender: mpsc::Sender<()>) {
        *self.watcher_lifecycle_lock_attempt.lock().unwrap() = Some(sender);
    }

    #[cfg(test)]
    pub(crate) fn test_managed_watcher_pids(&self) -> (Option<u32>, Option<u32>) {
        let window = self
            .window_watcher
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|watcher| watcher.child.id()));
        let afk = self
            .afk_watcher
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|watcher| watcher.child.id()));
        (window, afk)
    }

    fn start_watchers_with_timeout(
        &self,
        capture_titles: bool,
        timeout: StdDuration,
    ) -> Result<(), String> {
        let generation = self.capture_generation.load(Ordering::Acquire);
        if self.cancellation.is_cancelled() || self.capture_suspended.load(Ordering::Acquire) {
            return Err(CAPTURE_START_CANCELLED.into());
        }
        let port = self.ready_port().map_err(|error| {
            self.set_capture_issue(CaptureIssueCode::ServerUnavailable);
            error
        })?;
        // Never overwrite a live Child handle. A partial previous run must be
        // terminated before a full pair is spawned or it would become orphaned.
        self.stop_watchers_unlocked()?;
        let mut window_mode =
            self.resolve_window_capture_mode(capture_titles)
                .map_err(|error| {
                    self.set_capture_issue(CaptureIssueCode::WindowWatcherUnavailable);
                    error
                })?;
        let window_baseline = self.required_watcher_baseline(WatcherKind::Window)?;
        let afk_baseline = self.required_watcher_baseline(WatcherKind::Afk)?;
        if let Err(error) = self.start_watcher(WatcherKind::Window, port, window_mode, generation) {
            self.set_capture_issue(error.code);
            let _ = self.stop_watchers_unlocked();
            return Err(error.message);
        }
        if let Err(error) = self.start_watcher(
            WatcherKind::Afk,
            port,
            WindowCaptureMode::Stopped,
            generation,
        ) {
            self.set_capture_issue(error.code);
            let _ = self.stop_watchers_unlocked();
            return Err(error.message);
        }
        let baselines = [
            (WatcherKind::Window, window_baseline),
            (WatcherKind::Afk, afk_baseline),
        ];
        let mut result = self.await_watcher_progress(&baselines, timeout, generation);
        if result.as_ref().err().map(String::as_str) == Some(WINDOW_PERMISSION_REQUIRED) {
            window_mode = WindowCaptureMode::AppOnlyPermission;
            self.set_window_capture_mode(window_mode);
            self.start_watcher(WatcherKind::Window, port, window_mode, generation)
                .map_err(|error| {
                    self.set_capture_issue(error.code);
                    let _ = self.stop_watchers_unlocked();
                    error.message
                })?;
            result = self.await_watcher_progress(&baselines, timeout, generation);
        }
        if result.is_ok() {
            self.capture_titles.store(capture_titles, Ordering::Release);
            self.set_window_capture_mode(window_mode);
        }
        result
    }

    pub fn retry_watchers(&self, capture_titles: bool) -> Result<(), String> {
        let _lifecycle = self
            .watcher_lifecycle
            .lock()
            .map_err(|_| "No se pudo coordinar el capturador".to_string())?;
        let generation = self.capture_generation.load(Ordering::Acquire);
        if self.capture_cancelled(generation) {
            return Err(CAPTURE_START_CANCELLED.into());
        }
        let current_mode = self.window_capture_mode();
        if capture_titles && current_mode == WindowCaptureMode::AppOnlyPermission {
            let desired = self.resolve_window_capture_mode(true)?;
            if desired == WindowCaptureMode::AppOnlyPermission {
                #[cfg(all(target_os = "macos", not(test)))]
                if self.native_window_inspection().status == "running" {
                    return Ok(());
                }
                #[cfg(not(all(target_os = "macos", not(test))))]
                {
                    let inspection = inspect_watcher(
                        &self.window_watcher,
                        self.binary_path(WatcherKind::Window).exists(),
                        WatcherKind::Window,
                    );
                    if inspection.status == "running" {
                        return Ok(());
                    }
                }
            }
            let result = self.restart_window_watcher_unlocked(true, desired, generation);
            self.record_start_result(&result);
            return result;
        }
        let (window, afk) = self.watcher_statuses_with_timeout(WATCHER_STALE_TIMEOUT);
        if window == "running" && afk == "running" {
            return Ok(());
        }
        let result = self.start_watchers_with_timeout(capture_titles, WATCHER_REGISTRATION_TIMEOUT);
        self.record_start_result(&result);
        result
    }

    pub fn request_accessibility_permission(&self) -> Result<(), String> {
        #[cfg(all(target_os = "macos", not(test)))]
        {
            crate::macos_capture::request_accessibility_permission();
            return Ok(());
        }
        #[cfg(all(target_os = "macos", test))]
        {
            let helper = self.binary_path(WatcherKind::Window);
            let output = Command::new(&helper)
                .arg("--request-accessibility")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .output()
                .map_err(|error| format!("No se pudo solicitar Accesibilidad: {error}"))?;
            if output.status.success() || output.status.code() == Some(WINDOW_PERMISSION_EXIT) {
                return Ok(());
            }
            return Err(format!(
                "macOS no pudo solicitar Accesibilidad{}",
                diagnostic_suffix(&String::from_utf8_lossy(&output.stderr))
            ));
        }
        #[cfg(not(target_os = "macos"))]
        Err("Accesibilidad sólo está disponible en macOS".into())
    }

    pub fn restart_window_watcher(&self, capture_titles: bool) -> Result<(), String> {
        let _lifecycle = self
            .watcher_lifecycle
            .lock()
            .map_err(|_| "No se pudo coordinar el capturador".to_string())?;
        let generation = self.capture_generation.load(Ordering::Acquire);
        if self.capture_cancelled(generation) {
            return Err(CAPTURE_START_CANCELLED.into());
        }
        let mode = self.resolve_window_capture_mode(capture_titles)?;
        self.restart_window_watcher_unlocked(capture_titles, mode, generation)
    }

    fn restart_window_watcher_unlocked(
        &self,
        capture_titles: bool,
        mut mode: WindowCaptureMode,
        generation: u64,
    ) -> Result<(), String> {
        if self.capture_cancelled(generation) {
            return Err(CAPTURE_START_CANCELLED.into());
        }
        let port = self.ready_port().map_err(|error| {
            self.set_capture_issue(CaptureIssueCode::ServerUnavailable);
            error
        })?;
        let baseline = self.required_watcher_baseline(WatcherKind::Window)?;
        #[cfg(all(target_os = "macos", not(test)))]
        self.stop_native_window_worker()?;
        #[cfg(not(all(target_os = "macos", not(test))))]
        terminate_slot(&self.window_watcher)?;
        if let Err(error) = self.start_watcher(WatcherKind::Window, port, mode, generation) {
            self.set_capture_issue(error.code);
            return Err(error.message);
        }
        let mut result = self.await_watcher_progress(
            &[(WatcherKind::Window, baseline.clone())],
            WATCHER_REGISTRATION_TIMEOUT,
            generation,
        );
        if result.as_ref().err().map(String::as_str) == Some(WINDOW_PERMISSION_REQUIRED) {
            mode = WindowCaptureMode::AppOnlyPermission;
            self.set_window_capture_mode(mode);
            self.start_watcher(WatcherKind::Window, port, mode, generation)
                .map_err(|error| error.message)?;
            result = self.await_watcher_progress(
                &[(WatcherKind::Window, baseline)],
                WATCHER_REGISTRATION_TIMEOUT,
                generation,
            );
        }
        if result.is_ok() {
            self.capture_titles.store(capture_titles, Ordering::Release);
            self.set_window_capture_mode(mode);
        }
        result
    }

    fn ready_port(&self) -> Result<u16, String> {
        self.port_if_ready().ok_or_else(|| {
            self.server_error().unwrap_or_else(|| {
                "El servicio local todavía no está listo; se reintentará automáticamente".into()
            })
        })
    }

    fn window_capture_mode(&self) -> WindowCaptureMode {
        self.window_capture_mode
            .lock()
            .map(|mode| *mode)
            .unwrap_or(WindowCaptureMode::Stopped)
    }

    fn set_window_capture_mode(&self, mode: WindowCaptureMode) {
        if let Ok(mut current) = self.window_capture_mode.lock() {
            *current = mode;
        }
    }

    fn resolve_window_capture_mode(
        &self,
        capture_titles: bool,
    ) -> Result<WindowCaptureMode, String> {
        if !capture_titles {
            return Ok(WindowCaptureMode::AppOnlyPreference);
        }
        #[cfg(target_os = "macos")]
        {
            return self.accessibility_is_trusted().map(|trusted| {
                if trusted {
                    WindowCaptureMode::Full
                } else {
                    WindowCaptureMode::AppOnlyPermission
                }
            });
        }
        #[cfg(not(target_os = "macos"))]
        Ok(WindowCaptureMode::Full)
    }

    #[cfg(target_os = "macos")]
    fn accessibility_is_trusted(&self) -> Result<bool, String> {
        #[cfg(not(test))]
        {
            return Ok(crate::macos_capture::accessibility_is_trusted());
        }
        #[cfg(test)]
        {
            let helper = self.binary_path(WatcherKind::Window);
            if !helper.is_file() {
                return Err(format!(
                    "No se encontró el capturador de ventanas en {}",
                    helper.display()
                ));
            }
            let output = Command::new(&helper)
                .arg("--accessibility-status")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .output()
                .map_err(|error| format!("No se pudo comprobar Accesibilidad: {error}"))?;
            match output.status.code() {
                Some(0) => Ok(true),
                Some(WINDOW_PERMISSION_EXIT) => Ok(false),
                _ => Err(format!(
                    "No se pudo comprobar Accesibilidad{}",
                    diagnostic_suffix(&String::from_utf8_lossy(&output.stderr))
                )),
            }
        }
    }

    pub fn stop_watchers(&self) -> Result<(), String> {
        let _lifecycle = self
            .watcher_lifecycle
            .lock()
            .map_err(|_| "No se pudo coordinar el capturador".to_string())?;
        self.stop_watchers_unlocked()
    }

    fn stop_watchers_unlocked(&self) -> Result<(), String> {
        #[cfg(all(target_os = "macos", not(test)))]
        let window_result = self.stop_native_window_worker();
        #[cfg(not(all(target_os = "macos", not(test))))]
        let window_result = terminate_slot(&self.window_watcher);
        #[cfg(all(target_os = "macos", not(test)))]
        let afk_result = self.stop_native_afk_worker();
        #[cfg(not(all(target_os = "macos", not(test))))]
        let afk_result = terminate_slot(&self.afk_watcher);
        self.clear_watcher_health();
        self.set_window_capture_mode(WindowCaptureMode::Stopped);
        window_result.and(afk_result)
    }

    #[cfg(all(target_os = "macos", not(test)))]
    fn start_native_window_worker(&self, generation: u64) -> Result<(), WatcherStartError> {
        let mut slot = self.window_worker.lock().map_err(|_| {
            WatcherStartError::unavailable(
                WatcherKind::Window,
                "No se pudo coordinar la captura de ventanas".into(),
            )
        })?;
        if slot
            .as_ref()
            .is_some_and(|worker| worker.running.load(Ordering::Acquire))
        {
            return Ok(());
        }
        if let Some(worker) = slot.take() {
            worker.stop.store(true, Ordering::Release);
            let _ = worker.thread.join();
        }
        let datastore = self
            .datastore()
            .map_err(|message| WatcherStartError::unavailable(WatcherKind::Window, message))?;
        let bucket_id = self.watcher_bucket_id(WatcherKind::Window);
        let hostname = gethostname::gethostname().to_string_lossy().into_owned();
        let buckets = safe_datastore(|| datastore.get_buckets())
            .map_err(|message| WatcherStartError::unavailable(WatcherKind::Window, message))?;
        if !buckets.contains_key(&bucket_id) {
            let bucket = aw_models::Bucket {
                bid: None,
                id: bucket_id.clone(),
                _type: WatcherKind::Window.bucket_type().into(),
                client: WatcherKind::Window.binary_name().into(),
                hostname,
                created: Some(Utc::now()),
                data: serde_json::Map::new(),
                metadata: aw_models::BucketMetadata::default(),
                events: None,
                last_updated: None,
            };
            safe_datastore(|| datastore.create_bucket(&bucket))
                .map_err(|message| WatcherStartError::unavailable(WatcherKind::Window, message))?;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let worker_stop = Arc::clone(&stop);
        let worker_running = Arc::clone(&running);
        let capture_titles = Arc::clone(&self.capture_titles);
        let capture_issue = Arc::clone(&self.capture_issue);
        let window_mode = Arc::clone(&self.window_capture_mode);
        let cancellation = Arc::clone(&self.cancellation);
        let thread = std::thread::Builder::new()
            .name("enso-window-capture".into())
            .spawn(move || {
                while !worker_stop.load(Ordering::Acquire) && !cancellation.is_cancelled() {
                    let include_title = capture_titles.load(Ordering::Acquire);
                    let observation = crate::macos_capture::current_window(include_title);
                    let mode = if !include_title {
                        WindowCaptureMode::AppOnlyPreference
                    } else if observation.accessibility_trusted {
                        WindowCaptureMode::Full
                    } else {
                        WindowCaptureMode::AppOnlyPermission
                    };
                    if let Ok(mut current) = window_mode.lock() {
                        *current = mode;
                    }
                    if let Ok(mut issue) = capture_issue.lock() {
                        if mode == WindowCaptureMode::AppOnlyPermission {
                            *issue = Some(CaptureIssueCode::AccessibilityPermissionRequired);
                        } else if *issue == Some(CaptureIssueCode::AccessibilityPermissionRequired)
                        {
                            *issue = None;
                        }
                    }
                    let mut data = serde_json::Map::new();
                    data.insert("app".into(), serde_json::Value::String(observation.app));
                    data.insert(
                        "title".into(),
                        serde_json::Value::String(
                            observation.title.unwrap_or_else(|| "excluded".into()),
                        ),
                    );
                    let event = Event::new(Utc::now(), Duration::zero(), data);
                    if safe_datastore(|| {
                        datastore.heartbeat(&bucket_id, event, WINDOW_HEARTBEAT_PULSETIME_SECONDS)
                    })
                    .is_err()
                    {
                        if let Ok(mut issue) = capture_issue.lock() {
                            *issue = Some(CaptureIssueCode::WindowWatcherUnavailable);
                        }
                        break;
                    }
                    for _ in 0..20 {
                        if worker_stop.load(Ordering::Acquire) || cancellation.is_cancelled() {
                            break;
                        }
                        std::thread::sleep(StdDuration::from_millis(100));
                    }
                }
                worker_running.store(false, Ordering::Release);
            })
            .map_err(|error| {
                WatcherStartError::unavailable(
                    WatcherKind::Window,
                    format!("No se pudo iniciar la captura de ventanas: {error}"),
                )
            })?;
        if self.capture_cancelled(generation) {
            stop.store(true, Ordering::Release);
            let _ = thread.join();
            return Err(WatcherStartError::unavailable(
                WatcherKind::Window,
                CAPTURE_START_CANCELLED.into(),
            ));
        }
        *slot = Some(ManagedNativeWorker {
            stop,
            running,
            thread,
        });
        Ok(())
    }

    #[cfg(all(target_os = "macos", not(test)))]
    fn start_native_afk_worker(&self, generation: u64) -> Result<(), WatcherStartError> {
        let mut slot = self.afk_worker.lock().map_err(|_| {
            WatcherStartError::unavailable(
                WatcherKind::Afk,
                "No se pudo coordinar la captura de inactividad".into(),
            )
        })?;
        if slot
            .as_ref()
            .is_some_and(|worker| worker.running.load(Ordering::Acquire))
        {
            return Ok(());
        }
        if let Some(worker) = slot.take() {
            worker.stop.store(true, Ordering::Release);
            let _ = worker.thread.join();
        }
        let datastore = self
            .datastore()
            .map_err(|message| WatcherStartError::unavailable(WatcherKind::Afk, message))?;
        let bucket_id = self.watcher_bucket_id(WatcherKind::Afk);
        let hostname = gethostname::gethostname().to_string_lossy().into_owned();
        let buckets = safe_datastore(|| datastore.get_buckets())
            .map_err(|message| WatcherStartError::unavailable(WatcherKind::Afk, message))?;
        if !buckets.contains_key(&bucket_id) {
            let bucket = aw_models::Bucket {
                bid: None,
                id: bucket_id.clone(),
                _type: WatcherKind::Afk.bucket_type().into(),
                client: WatcherKind::Afk.binary_name().into(),
                hostname,
                created: Some(Utc::now()),
                data: serde_json::Map::new(),
                metadata: aw_models::BucketMetadata::default(),
                events: None,
                last_updated: None,
            };
            safe_datastore(|| datastore.create_bucket(&bucket))
                .map_err(|message| WatcherStartError::unavailable(WatcherKind::Afk, message))?;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let worker_stop = Arc::clone(&stop);
        let worker_running = Arc::clone(&running);
        let capture_issue = Arc::clone(&self.capture_issue);
        let cancellation = Arc::clone(&self.cancellation);
        let thread = std::thread::Builder::new()
            .name("enso-afk-capture".into())
            .spawn(move || {
                while !worker_stop.load(Ordering::Acquire) && !cancellation.is_cancelled() {
                    let status =
                        if crate::macos_capture::idle_seconds() >= AFK_IDLE_THRESHOLD_SECONDS {
                            "afk"
                        } else {
                            "not-afk"
                        };
                    let mut data = serde_json::Map::new();
                    data.insert("status".into(), serde_json::Value::String(status.into()));
                    let event = Event::new(Utc::now(), Duration::zero(), data);
                    if safe_datastore(|| {
                        datastore.heartbeat(&bucket_id, event, AFK_HEARTBEAT_PULSETIME_SECONDS)
                    })
                    .is_err()
                    {
                        if let Ok(mut issue) = capture_issue.lock() {
                            *issue = Some(CaptureIssueCode::AfkWatcherUnavailable);
                        }
                        break;
                    }
                    for _ in 0..50 {
                        if worker_stop.load(Ordering::Acquire) || cancellation.is_cancelled() {
                            break;
                        }
                        std::thread::sleep(StdDuration::from_millis(100));
                    }
                }
                worker_running.store(false, Ordering::Release);
            })
            .map_err(|error| {
                WatcherStartError::unavailable(
                    WatcherKind::Afk,
                    format!("No se pudo iniciar la captura de inactividad: {error}"),
                )
            })?;
        if self.capture_cancelled(generation) {
            stop.store(true, Ordering::Release);
            let _ = thread.join();
            return Err(WatcherStartError::unavailable(
                WatcherKind::Afk,
                CAPTURE_START_CANCELLED.into(),
            ));
        }
        *slot = Some(ManagedNativeWorker {
            stop,
            running,
            thread,
        });
        Ok(())
    }

    #[cfg(all(target_os = "macos", not(test)))]
    fn stop_native_window_worker(&self) -> Result<(), String> {
        let worker = self
            .window_worker
            .lock()
            .map_err(|_| "No se pudo detener la captura de ventanas".to_string())?
            .take();
        let Some(worker) = worker else {
            return Ok(());
        };
        worker.stop.store(true, Ordering::Release);
        worker
            .thread
            .join()
            .map_err(|_| "La captura de ventanas no terminó correctamente".into())
    }

    #[cfg(all(target_os = "macos", not(test)))]
    fn stop_native_afk_worker(&self) -> Result<(), String> {
        let worker = self
            .afk_worker
            .lock()
            .map_err(|_| "No se pudo detener la captura de inactividad".to_string())?
            .take();
        let Some(worker) = worker else {
            return Ok(());
        };
        worker.stop.store(true, Ordering::Release);
        worker
            .thread
            .join()
            .map_err(|_| "La captura de inactividad no terminó correctamente".into())
    }

    #[cfg(all(target_os = "macos", not(test)))]
    fn native_window_inspection(&self) -> WatcherInspection {
        let Ok(slot) = self.window_worker.lock() else {
            return WatcherInspection {
                status: "error".into(),
                issue: Some(CaptureIssueCode::WindowWatcherUnavailable),
                exit_code: None,
            };
        };
        match slot.as_ref() {
            Some(worker) if worker.running.load(Ordering::Acquire) => WatcherInspection {
                status: "running".into(),
                issue: None,
                exit_code: None,
            },
            Some(_) => WatcherInspection {
                status: "stopped".into(),
                issue: Some(CaptureIssueCode::WindowWatcherUnavailable),
                exit_code: None,
            },
            None => WatcherInspection {
                status: "stopped".into(),
                issue: None,
                exit_code: None,
            },
        }
    }

    #[cfg(all(target_os = "macos", not(test)))]
    fn native_afk_inspection(&self) -> WatcherInspection {
        let Ok(slot) = self.afk_worker.lock() else {
            return WatcherInspection {
                status: "error".into(),
                issue: Some(CaptureIssueCode::AfkWatcherUnavailable),
                exit_code: None,
            };
        };
        match slot.as_ref() {
            Some(worker) if worker.running.load(Ordering::Acquire) => WatcherInspection {
                status: "running".into(),
                issue: None,
                exit_code: None,
            },
            Some(_) => WatcherInspection {
                status: "stopped".into(),
                issue: Some(CaptureIssueCode::AfkWatcherUnavailable),
                exit_code: None,
            },
            None => WatcherInspection {
                status: "stopped".into(),
                issue: None,
                exit_code: None,
            },
        }
    }

    pub fn watcher_statuses(&self) -> (String, String) {
        let Ok(_lifecycle) = self.watcher_lifecycle.lock() else {
            self.set_capture_issue(CaptureIssueCode::ServerUnavailable);
            return ("stopped".into(), "stopped".into());
        };
        self.watcher_statuses_with_timeout(WATCHER_STALE_TIMEOUT)
    }

    fn watcher_statuses_with_timeout(&self, stale_timeout: StdDuration) -> (String, String) {
        if self.server_error().is_some() {
            self.set_capture_issue(CaptureIssueCode::ServerUnavailable);
            let _ = self.stop_watchers_unlocked();
        }
        #[cfg(all(target_os = "macos", not(test)))]
        let window = self.native_window_inspection();
        #[cfg(not(all(target_os = "macos", not(test))))]
        let window = inspect_watcher(
            &self.window_watcher,
            self.binary_path(WatcherKind::Window).exists(),
            WatcherKind::Window,
        );
        #[cfg(all(target_os = "macos", not(test)))]
        let afk = self.native_afk_inspection();
        #[cfg(not(all(target_os = "macos", not(test))))]
        let afk = inspect_watcher(
            &self.afk_watcher,
            self.binary_path(WatcherKind::Afk).exists(),
            WatcherKind::Afk,
        );
        #[cfg(target_os = "macos")]
        if window.exit_code == Some(WINDOW_PERMISSION_EXIT) {
            self.set_window_capture_mode(WindowCaptureMode::AppOnlyPermission);
            self.clear_capture_issue();
            return ("stopped".into(), afk.status);
        }
        if let Some(issue) = window.issue.or(afk.issue) {
            self.set_capture_issue(issue);
            let _ = self.stop_watchers_unlocked();
            return ("stopped".into(), "stopped".into());
        }
        if window.status == "running" && afk.status == "running" {
            for kind in [WatcherKind::Window, WatcherKind::Afk] {
                match self.refresh_watcher_health(kind, stale_timeout) {
                    Ok(false) => {}
                    Ok(true) | Err(_) => {
                        self.fail_capture(kind.unavailable_code());
                        return ("stopped".into(), "stopped".into());
                    }
                }
            }
        }
        (window.status, afk.status)
    }

    pub fn timeline(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        settings: &SettingsV1,
        checkins: &[CheckInV1],
        rules: &[ClassificationRuleV1],
        overrides: &[ClassificationOverrideV1],
    ) -> Result<Vec<TimelineBlockV1>, String> {
        if to <= from {
            return Err("El fin del rango debe ser posterior al inicio".into());
        }
        let datastore = self.datastore()?;
        let buckets = safe_datastore(|| datastore.get_buckets())?;
        let window_buckets: Vec<_> = buckets
            .values()
            .filter(|bucket| bucket._type == "currentwindow")
            .collect();
        let afk_buckets: Vec<_> = buckets
            .values()
            .filter(|bucket| bucket._type == "afkstatus")
            .collect();

        let mut afk_intervals = Vec::new();
        for bucket in afk_buckets {
            let events =
                safe_datastore(|| datastore.get_events(&bucket.id, Some(from), Some(to), None))?;
            for event in events {
                if string_field(&event, "status").as_deref() != Some("afk")
                    || event.duration <= Duration::zero()
                {
                    continue;
                }
                let start = event.timestamp.max(from);
                let end = event.calculate_endtime().min(to);
                if end > start {
                    afk_intervals.push((start, end));
                }
            }
        }
        afk_intervals.sort_by_key(|(start, _)| *start);

        let answered_checkins = checkins
            .iter()
            .filter(|item| item.status == "answered")
            .filter_map(|item| {
                let start = crate::models::parse_utc(&item.interval_start).ok()?;
                let end = crate::models::parse_utc(&item.interval_end).ok()?;
                (end > start).then_some((item, start, end))
            })
            .collect::<Vec<_>>();
        let override_intervals = overrides
            .iter()
            .filter_map(|item| {
                let start = crate::models::parse_utc(&item.event_start).ok()?;
                let end = crate::models::parse_utc(&item.event_end).ok()?;
                (end > start).then_some((item, start, end))
            })
            .collect::<Vec<_>>();

        let mut window_events = Vec::new();
        for bucket in window_buckets {
            for event in
                safe_datastore(|| datastore.get_events(&bucket.id, Some(from), Some(to), None))?
            {
                window_events.push((bucket.id.clone(), event));
            }
        }
        window_events.sort_by_key(|(_, event)| event.timestamp);

        let mut blocks = Vec::new();
        for (bucket_id, event) in window_events {
            let event_start = event.timestamp.max(from);
            let event_end = event.calculate_endtime().min(to);
            if event_end <= event_start {
                continue;
            }
            let app =
                string_field(&event, "app").unwrap_or_else(|| "Aplicación desconocida".into());
            let title = if settings.capture_window_titles {
                string_field(&event, "title").filter(|title| title != "excluded")
            } else {
                None
            };
            let base_block_id = format!(
                "{}-{}",
                bucket_id,
                event
                    .id
                    .unwrap_or_else(|| event.timestamp.timestamp_millis())
            );
            let automatic = classify(&app, title.as_deref(), rules, &settings.app_rules);

            let afk_start = afk_intervals.partition_point(|(_, end)| *end <= event_start);
            let relevant_afk = afk_intervals[afk_start..]
                .iter()
                .take_while(|(start, _)| *start < event_end)
                .copied()
                .collect::<Vec<_>>();
            let relevant_checkins = answered_checkins
                .iter()
                .filter(|(_, start, end)| ranges_overlap(*start, *end, event_start, event_end))
                .copied()
                .collect::<Vec<_>>();
            let relevant_overrides = override_intervals
                .iter()
                .filter(|(item, start, end)| {
                    source_block_id(&item.block_id) == base_block_id
                        && ranges_overlap(*start, *end, event_start, event_end)
                })
                .copied()
                .collect::<Vec<_>>();
            let mut cuts = vec![event_start, event_end];
            for (_, start, end) in &relevant_checkins {
                cuts.push((*start).max(event_start));
                cuts.push((*end).min(event_end));
            }
            for (_, start, end) in &relevant_overrides {
                cuts.push((*start).max(event_start));
                cuts.push((*end).min(event_end));
            }
            for (start, end) in &relevant_afk {
                cuts.push((*start).max(event_start));
                cuts.push((*end).min(event_end));
            }
            cuts.sort_unstable();
            cuts.dedup();
            let segmented = cuts.len() > 2;
            for interval in cuts.windows(2) {
                let segment_start = interval[0];
                let segment_end = interval[1];
                if segment_end <= segment_start {
                    continue;
                }
                let id = if segmented {
                    format!(
                        "{}@{}-{}",
                        base_block_id,
                        segment_start.timestamp_millis(),
                        segment_end.timestamp_millis()
                    )
                } else {
                    base_block_id.clone()
                };
                let matched_override = relevant_overrides
                    .iter()
                    .filter(|(_, start, end)| {
                        ranges_overlap(*start, *end, segment_start, segment_end)
                    })
                    .max_by_key(|(item, start, end)| {
                        (
                            item.block_id == id,
                            std::cmp::Reverse((*end - *start).num_milliseconds()),
                            item.updated_at.as_str(),
                            item.block_id.as_str(),
                        )
                    });
                let matched_checkin = relevant_checkins.iter().find(|(_, start, end)| {
                    ranges_overlap(*start, *end, segment_start, segment_end)
                });
                let is_afk = relevant_afk
                    .iter()
                    .any(|(start, end)| ranges_overlap(*start, *end, segment_start, segment_end));
                let (
                    category,
                    activity_label,
                    source,
                    check_in_id,
                    confidence,
                    classification_source,
                    rule_id,
                ) = if let Some((item, _, _)) = matched_override {
                    (
                        item.category.clone(),
                        item.activity_label.clone(),
                        "window".into(),
                        None,
                        1.0,
                        "override".into(),
                        None,
                    )
                } else if let Some((checkin, _, _)) = matched_checkin {
                    (
                        checkin.category.clone().unwrap_or_else(|| "unknown".into()),
                        checkin.activity_label.clone(),
                        "checkin".into(),
                        Some(checkin.id.clone()),
                        1.0,
                        "checkin".into(),
                        None,
                    )
                } else if is_afk {
                    (
                        "break".into(),
                        None,
                        "window".into(),
                        None,
                        1.0,
                        "afk".into(),
                        None,
                    )
                } else {
                    (
                        automatic.category.clone(),
                        automatic.activity_label.clone(),
                        "window".into(),
                        None,
                        automatic.confidence,
                        automatic.source.clone(),
                        automatic.rule_id.clone(),
                    )
                };
                blocks.push(TimelineBlockV1 {
                    id,
                    start: segment_start.to_rfc3339(),
                    end: segment_end.to_rfc3339(),
                    duration_seconds: (segment_end - segment_start).num_seconds().max(0),
                    app: app.clone(),
                    title: title.clone(),
                    activity_label,
                    category,
                    confidence,
                    afk: is_afk,
                    source,
                    check_in_id,
                    classification_source,
                    rule_id,
                    correctable: true,
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
                        activity_label: checkin.activity_label.clone(),
                        category: checkin.category.clone().unwrap_or_else(|| "unknown".into()),
                        confidence: 1.0,
                        afk: false,
                        source: "checkin".into(),
                        check_in_id: Some(checkin.id.clone()),
                        classification_source: "checkin".into(),
                        rule_id: None,
                        correctable: false,
                    });
                }
            }
        }
        blocks.sort_by(|left, right| left.start.cmp(&right.start));
        Ok(blocks)
    }

    pub fn window_event_interval(
        &self,
        block_id: &str,
    ) -> Result<(DateTime<Utc>, DateTime<Utc>), String> {
        let (base_id, projected_interval) = block_id
            .split_once('@')
            .map_or((block_id, None), |(base, interval)| (base, Some(interval)));
        let (bucket_id, event_id) = base_id
            .rsplit_once('-')
            .ok_or_else(|| "La sesión no es válida".to_string())?;
        let event_id = event_id
            .parse::<i64>()
            .map_err(|_| "La sesión no es válida".to_string())?;
        let datastore = self.datastore()?;
        let bucket = safe_datastore(|| datastore.get_bucket(bucket_id))?;
        if bucket._type != "currentwindow" {
            return Err("La sesión no corresponde a una ventana".into());
        }
        let event = safe_datastore(|| datastore.get_event(bucket_id, event_id))?;
        let event_end = event.calculate_endtime();
        if event_end <= event.timestamp {
            return Err("La sesión todavía no tiene duración suficiente".into());
        }
        if let Some(interval) = projected_interval {
            let (start, projected_end) = interval
                .split_once('-')
                .ok_or_else(|| "La sesión no es válida".to_string())?;
            let start = DateTime::<Utc>::from_timestamp_millis(
                start.parse::<i64>().map_err(|_| "La sesión no es válida")?,
            )
            .ok_or_else(|| "La sesión no es válida".to_string())?;
            let projected_end = DateTime::<Utc>::from_timestamp_millis(
                projected_end
                    .parse::<i64>()
                    .map_err(|_| "La sesión no es válida")?,
            )
            .ok_or_else(|| "La sesión no es válida".to_string())?;
            if start < event.timestamp || projected_end > event_end || projected_end <= start {
                return Err("La sesión no es válida".into());
            }
            return Ok((start, projected_end));
        }
        Ok((event.timestamp, event_end))
    }

    pub fn interval_reason(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        settings: &SettingsV1,
        rules: &[ClassificationRuleV1],
    ) -> Result<Option<String>, String> {
        let blocks = self.timeline(from, to, settings, &[], rules, &[])?;
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
        let datastore = self.datastore()?;
        let buckets = safe_datastore(|| datastore.get_buckets())?;
        Ok(buckets
            .values()
            .filter_map(|bucket| bucket.metadata.end)
            .max()
            .map(|date| date.to_rfc3339()))
    }

    pub fn delete_all_activity(&self) -> Result<(), String> {
        let datastore = self.datastore()?;
        let buckets = safe_datastore(|| datastore.get_buckets())?;
        for bucket_id in buckets.keys() {
            safe_datastore(|| datastore.delete_bucket(bucket_id))?;
        }
        safe_datastore(|| datastore.force_commit())?;
        self.recreate_datastore_securely()
    }

    pub fn delete_queued_activity(&self) -> Result<(), String> {
        // Every queue and log used by either the pinned legacy client or a
        // profile-aware future client lives below this one Ensō-owned root.
        if self.profile.root.exists() {
            delete_private_transient_data(&self.profile.root)?;
            self.profile.prepare()?;
        }
        Ok(())
    }

    pub fn prune_activity(&self, retention_days: Option<u16>) -> Result<usize, String> {
        let Some(days) = retention_days else {
            return Ok(0);
        };
        let cutoff = Utc::now() - Duration::days(i64::from(days));
        let (window_status, afk_status) = self.watcher_statuses();
        let restart_watchers = window_status == "running" && afk_status == "running";
        let datastore = self.datastore()?;
        let buckets = safe_datastore(|| datastore.get_buckets())?;
        let mut deleted = 0;
        for bucket_id in buckets.keys() {
            let events = safe_datastore(|| {
                datastore.get_events_unclipped(bucket_id, None, Some(cutoff), None)
            })?;
            let event_ids: Vec<i64> = events
                .iter()
                .filter(|event| event.calculate_endtime() <= cutoff)
                .filter_map(|event| event.id)
                .collect();
            if !event_ids.is_empty() {
                deleted += event_ids.len();
                safe_datastore(|| datastore.delete_events_by_id(bucket_id, event_ids))?;
            }
        }
        if deleted > 0 {
            safe_datastore(|| datastore.force_commit())?;
            self.recreate_datastore_securely()?;
            if restart_watchers {
                self.start_watchers(self.capture_titles.load(Ordering::Acquire))?;
            }
        }
        Ok(deleted)
    }

    fn start_watcher(
        &self,
        kind: WatcherKind,
        port: u16,
        window_mode: WindowCaptureMode,
        generation: u64,
    ) -> Result<(), WatcherStartError> {
        #[cfg(all(target_os = "macos", not(test)))]
        let _ = (port, window_mode);
        if self.capture_cancelled(generation) {
            return Err(WatcherStartError::unavailable(
                kind,
                CAPTURE_START_CANCELLED.into(),
            ));
        }
        #[cfg(all(target_os = "macos", not(test)))]
        match kind {
            WatcherKind::Window => return self.start_native_window_worker(generation),
            WatcherKind::Afk => return self.start_native_afk_worker(generation),
        }
        #[cfg(any(not(target_os = "macos"), test))]
        {
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
            let stderr_path = self
                .profile
                .diagnostics
                .join(format!("{}.stderr.log", kind.binary_name()));
            let stderr = private_log_file(&stderr_path)
                .map_err(|message| WatcherStartError::unavailable(kind, message))?;
            let stdout = stderr.try_clone().map_err(|error| {
                WatcherStartError::unavailable(
                    kind,
                    format!("No se pudo preparar el diagnóstico privado: {error}"),
                )
            })?;
            let mut command = Command::new(&binary);
            #[cfg(target_os = "macos")]
            {
                if matches!(kind, WatcherKind::Afk) {
                    command.arg("--afk");
                }
                command
                    .arg(format!("http://127.0.0.1:{port}"))
                    .arg(self.watcher_bucket_id(kind))
                    .arg(gethostname::gethostname())
                    .arg(kind.binary_name())
                    .env(ACTIVITYWATCH_API_KEY_ENV, &self.api_key);
                if matches!(kind, WatcherKind::Window) && window_mode != WindowCaptureMode::Full {
                    command.arg("--exclude-title");
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                command
                    .arg("--host")
                    .arg("127.0.0.1")
                    .arg("--port")
                    .arg(port.to_string());
                if matches!(kind, WatcherKind::Window) && window_mode != WindowCaptureMode::Full {
                    command.arg("--exclude-title");
                }
            }
            command
                .stdin(Stdio::null())
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr));
            self.profile.configure(&mut command);
            #[cfg(test)]
            command
                .env("ENSO_TEST_API_KEY", &self.api_key)
                .env("ENSO_TEST_BUCKET_ID", self.watcher_bucket_id(kind))
                .env("ENSO_TEST_BUCKET_TYPE", kind.bucket_type())
                .env("ENSO_TEST_CLIENT_NAME", kind.binary_name())
                .env(
                    "ENSO_TEST_HOSTNAME",
                    gethostname::gethostname().to_string_lossy().as_ref(),
                )
                .env("ENSO_TEST_PORT", port.to_string())
                .env(
                    "ENSO_TEST_WINDOW_MODE",
                    match window_mode {
                        WindowCaptureMode::Full => "full",
                        WindowCaptureMode::AppOnlyPreference => "app-only-preference",
                        WindowCaptureMode::AppOnlyPermission => "app-only-permission",
                        WindowCaptureMode::Stopped => "stopped",
                    },
                );
            if self.capture_cancelled(generation) {
                return Err(WatcherStartError::unavailable(
                    kind,
                    CAPTURE_START_CANCELLED.into(),
                ));
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
            // Cancellation can race the final pre-spawn check. Publishing the child
            // first and revalidating closes the only window where stop could miss it.
            if self.capture_cancelled(generation) {
                let _ = terminate_slot(slot);
                return Err(WatcherStartError::unavailable(
                    kind,
                    CAPTURE_START_CANCELLED.into(),
                ));
            }
            Ok(())
        }
    }

    fn capture_cancelled(&self, generation: u64) -> bool {
        self.cancellation.is_cancelled()
            || self.capture_suspended.load(Ordering::Acquire)
            || self.capture_generation.load(Ordering::Acquire) != generation
    }

    fn await_watcher_progress(
        &self,
        baselines: &[(WatcherKind, WatcherBaseline)],
        timeout: StdDuration,
        generation: u64,
    ) -> Result<(), String> {
        let deadline = std::time::Instant::now() + timeout;
        let mut healthy = vec![false; baselines.len()];
        loop {
            if self.cancellation.is_cancelled()
                || self.capture_suspended.load(Ordering::Acquire)
                || self.capture_generation.load(Ordering::Acquire) != generation
            {
                self.stop_watchers_unlocked()?;
                return Err(CAPTURE_START_CANCELLED.into());
            }
            for (index, (kind, baseline)) in baselines.iter().enumerate() {
                if healthy[index] {
                    continue;
                }
                let slot = match kind {
                    WatcherKind::Window => &self.window_watcher,
                    WatcherKind::Afk => &self.afk_watcher,
                };
                if let Some(status) = watcher_exit_status(slot).map_err(|message| {
                    self.fail_capture(kind.unavailable_code());
                    message
                })? {
                    #[cfg(target_os = "macos")]
                    if matches!(kind, WatcherKind::Window)
                        && status.code() == Some(WINDOW_PERMISSION_EXIT)
                    {
                        let _ = terminate_slot(slot);
                        self.set_window_capture_mode(WindowCaptureMode::AppOnlyPermission);
                        self.clear_capture_issue();
                        return Err(WINDOW_PERMISSION_REQUIRED.into());
                    }
                    let stderr_path = self
                        .profile
                        .diagnostics
                        .join(format!("{}.stderr.log", kind.binary_name()));
                    let error =
                        WatcherStartError::from_exit(*kind, status, &read_diagnostic(&stderr_path));
                    self.fail_capture(error.code);
                    return Err(error.message);
                }
                healthy[index] = match self.watcher_progressed(baseline) {
                    Ok(progressed) => progressed,
                    Err(error) => {
                        self.fail_capture(kind.unavailable_code());
                        return Err(error);
                    }
                };
            }
            if healthy.iter().all(|value| *value) {
                for (kind, baseline) in baselines {
                    if let Err(error) = self.record_watcher_health(*kind, baseline.clone()) {
                        self.fail_capture(kind.unavailable_code());
                        return Err(error);
                    }
                }
                self.clear_capture_issue();
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                let failed_kind = baselines
                    .iter()
                    .zip(&healthy)
                    .find_map(|((kind, _), is_healthy)| (!is_healthy).then_some(*kind))
                    .unwrap_or(WatcherKind::Window);
                self.fail_capture(failed_kind.unavailable_code());
                return Err(format!(
                    "El capturador {} siguió vivo, pero no registró actividad nueva en el servicio privado a tiempo",
                    failed_kind.label()
                ));
            }
            if self.cancellation.wait_timeout(StdDuration::from_millis(50))
                || self.capture_suspended.load(Ordering::Acquire)
                || self.capture_generation.load(Ordering::Acquire) != generation
            {
                self.stop_watchers_unlocked()?;
                return Err(CAPTURE_START_CANCELLED.into());
            }
        }
    }

    fn watcher_bucket_id(&self, kind: WatcherKind) -> String {
        format!(
            "{}_{}",
            kind.binary_name(),
            gethostname::gethostname().to_string_lossy()
        )
    }

    fn watcher_baseline(&self, kind: WatcherKind) -> Result<WatcherBaseline, String> {
        let bucket_id = self.watcher_bucket_id(kind);
        Ok(WatcherBaseline {
            snapshot: self.bucket_snapshot(&bucket_id)?,
            bucket_id,
            client: kind.binary_name(),
            hostname: gethostname::gethostname().to_string_lossy().into_owned(),
            bucket_type: kind.bucket_type(),
        })
    }

    fn required_watcher_baseline(&self, kind: WatcherKind) -> Result<WatcherBaseline, String> {
        self.watcher_baseline(kind).map_err(|error| {
            self.fail_capture(kind.unavailable_code());
            error
        })
    }

    fn watcher_progressed(&self, baseline: &WatcherBaseline) -> Result<bool, String> {
        let current = self.bucket_snapshot(&baseline.bucket_id)?;
        Ok(match (&baseline.snapshot, current) {
            (None, Some(after)) => baseline.matches(&after),
            (Some(before), Some(after)) => baseline.matches(&after) && before != &after,
            _ => false,
        })
    }

    fn record_watcher_health(
        &self,
        kind: WatcherKind,
        baseline: WatcherBaseline,
    ) -> Result<(), String> {
        let snapshot = self
            .bucket_snapshot(&baseline.bucket_id)?
            .filter(|snapshot| baseline.matches(snapshot))
            .ok_or_else(|| {
                format!(
                    "El capturador {} no registró su bucket privado esperado",
                    kind.label()
                )
            })?;
        *self
            .health_slot(kind)
            .lock()
            .map_err(|_| "No se pudo registrar la salud del capturador".to_string())? =
            Some(WatcherHealth {
                baseline,
                last_snapshot: snapshot,
                last_progress: Instant::now(),
            });
        Ok(())
    }

    fn refresh_watcher_health(
        &self,
        kind: WatcherKind,
        stale_timeout: StdDuration,
    ) -> Result<bool, String> {
        let mut slot = self
            .health_slot(kind)
            .lock()
            .map_err(|_| "No se pudo consultar la salud del capturador".to_string())?;
        let Some(health) = slot.as_mut() else {
            // A watcher can be observed between spawn and its verified first
            // heartbeat. Startup verification remains authoritative then.
            return Ok(false);
        };
        if let Some(snapshot) = self.bucket_snapshot(&health.baseline.bucket_id)? {
            if health.baseline.matches(&snapshot) && snapshot != health.last_snapshot {
                health.last_snapshot = snapshot;
                health.last_progress = Instant::now();
                return Ok(false);
            }
        }
        Ok(health.last_progress.elapsed() >= stale_timeout)
    }

    fn health_slot(&self, kind: WatcherKind) -> &Mutex<Option<WatcherHealth>> {
        match kind {
            WatcherKind::Window => &self.window_health,
            WatcherKind::Afk => &self.afk_health,
        }
    }

    fn clear_watcher_health(&self) {
        for kind in [WatcherKind::Window, WatcherKind::Afk] {
            if let Ok(mut slot) = self.health_slot(kind).lock() {
                *slot = None;
            }
        }
    }

    fn bucket_snapshot(&self, bucket_id: &str) -> Result<Option<BucketSnapshot>, String> {
        let datastore = self.datastore()?;
        let buckets = safe_datastore(|| datastore.get_buckets())?;
        let Some(bucket) = buckets.get(bucket_id) else {
            return Ok(None);
        };
        let event_count = safe_datastore(|| datastore.get_event_count(bucket_id, None, None))?;
        Ok(Some(BucketSnapshot {
            client: bucket.client.clone(),
            hostname: bucket.hostname.clone(),
            bucket_type: bucket._type.clone(),
            created: bucket.created,
            last_updated: bucket.last_updated,
            event_end: bucket.metadata.end,
            event_count,
        }))
    }

    fn fail_capture(&self, issue: CaptureIssueCode) {
        self.set_capture_issue(issue);
        let _ = self.stop_watchers_unlocked();
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

    fn recreate_datastore_securely(&self) -> Result<(), String> {
        let _lifecycle = self
            .watcher_lifecycle
            .lock()
            .map_err(|_| "No se pudo coordinar el mantenimiento".to_string())?;
        if self.cancellation.is_cancelled() {
            return Err("El runtime de captura ya se cerró".into());
        }
        // The vendored datastore keeps an immediate transaction open between
        // requests. Stop its only server consumer and close the worker before
        // VACUUM so secure deletion is real instead of a best-effort no-op.
        self.stop_watchers_unlocked()?;
        self.shutdown_server();
        let datastore = self
            .datastore
            .lock()
            .map_err(|_| "No se pudo cerrar el almacén de actividad".to_string())?
            .take()
            .ok_or_else(|| "El almacén de actividad ya está en mantenimiento".to_string())?;
        datastore.close();

        let compact_result = compact_activity_database(&self.database_path);
        let database = self
            .database_path
            .to_str()
            .ok_or_else(|| "La ruta del almacén de actividad no es válida".to_string())?
            .to_string();
        let reopened = catch_unwind(AssertUnwindSafe(|| Datastore::new(database, false)))
            .map_err(|_| "No se pudo reabrir el almacén de actividad".to_string())?;
        *self
            .datastore
            .lock()
            .map_err(|_| "No se pudo reabrir el almacén de actividad".to_string())? =
            Some(reopened);
        let launch_result = self.launch_server();
        if let Err(error) = launch_result {
            self.set_capture_issue(CaptureIssueCode::ServerUnavailable);
            return Err(error);
        }
        compact_result
    }

    pub fn shutdown(&self) -> Result<(), String> {
        let request_result = self.request_shutdown();
        if self.shutdown_started.swap(true, Ordering::AcqRel) {
            return request_result;
        }
        let _lifecycle = self
            .watcher_lifecycle
            .lock()
            .map_err(|_| "No se pudo coordinar el cierre del capturador".to_string())?;
        let mut errors = Vec::new();
        if let Err(error) = request_result {
            errors.push(error);
        }
        self.shutdown_server();
        match self.datastore.lock() {
            Ok(mut slot) => {
                if let Some(datastore) = slot.take() {
                    if catch_unwind(AssertUnwindSafe(|| datastore.close())).is_err() {
                        errors.push("No se pudo cerrar el worker del almacén de actividad".into());
                    }
                }
            }
            Err(_) => errors.push("No se pudo cerrar el almacén de actividad".into()),
        }
        if let Err(error) = compact_activity_database(&self.database_path) {
            errors.push(error);
        }
        if let Err(error) = checkpoint_and_remove_sqlite_sidecars(&self.database_path) {
            errors.push(error);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    pub fn request_shutdown(&self) -> Result<(), String> {
        self.cancellation.cancel();
        self.capture_generation.fetch_add(1, Ordering::AcqRel);
        self.stop_watchers_unlocked()
    }

    pub(crate) fn cancel_pending_capture(&self) -> Result<(), String> {
        self.capture_suspended.store(true, Ordering::Release);
        self.capture_generation.fetch_add(1, Ordering::AcqRel);
        self.stop_watchers_unlocked()
    }

    pub(crate) fn resume_pending_capture(&self) {
        self.capture_suspended.store(false, Ordering::Release);
    }

    pub(crate) fn shutdown_requested(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    #[cfg(test)]
    pub(crate) fn seed_test_activity(&self, marker: &str) -> Result<(), String> {
        let datastore = self.datastore()?;
        let bucket_id = "enso-test-private-delete";
        safe_datastore(|| {
            datastore.create_bucket(&aw_models::Bucket {
                bid: None,
                id: bucket_id.into(),
                _type: "currentwindow".into(),
                client: "enso-test".into(),
                hostname: "enso-test".into(),
                created: None,
                data: serde_json::Map::new(),
                metadata: aw_models::BucketMetadata::default(),
                events: None,
                last_updated: None,
            })
        })?;
        let mut data = serde_json::Map::new();
        data.insert("title".into(), serde_json::Value::String(marker.into()));
        let events = [Event::new(Utc::now(), Duration::seconds(1), data)];
        safe_datastore(|| datastore.insert_events(bucket_id, &events))?;
        safe_datastore(|| datastore.force_commit())
    }

    #[cfg(test)]
    pub(crate) fn test_activity_is_empty(&self) -> Result<bool, String> {
        let datastore = self.datastore()?;
        Ok(safe_datastore(|| datastore.get_buckets())?.is_empty())
    }

    #[cfg(test)]
    pub(crate) fn test_profile_root(&self) -> PathBuf {
        self.profile.root.clone()
    }

    #[cfg(test)]
    pub(crate) fn test_database_path(&self) -> PathBuf {
        self.database_path.clone()
    }

    #[cfg(test)]
    pub(crate) fn test_watchers_are_stopped(&self) -> bool {
        self.window_watcher
            .lock()
            .map(|slot| slot.is_none())
            .unwrap_or(false)
            && self
                .afk_watcher
                .lock()
                .map(|slot| slot.is_none())
                .unwrap_or(false)
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
        if let Ok(mut slot) = self.server_shutdown.lock() {
            *slot = None;
        }
        self.server_ready.store(false, Ordering::Release);
    }

    fn binary_path(&self, kind: WatcherKind) -> PathBuf {
        let name = kind.binary_name();
        let directory = self.resource_dir.join("activitywatch").join(name);
        #[cfg(target_os = "macos")]
        let binary = directory.join(match kind {
            WatcherKind::Window => WINDOW_HELPER_BINARY,
            #[cfg(test)]
            WatcherKind::Afk => AFK_HELPER_BINARY,
            #[cfg(not(test))]
            WatcherKind::Afk => "native-afk-capture",
        });
        #[cfg(target_os = "windows")]
        let binary = directory.join(format!("{name}.exe"));
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        let binary = directory.join(name);
        binary
    }
}

impl WatcherBaseline {
    fn matches(&self, snapshot: &BucketSnapshot) -> bool {
        snapshot.client == self.client
            && snapshot.hostname == self.hostname
            && snapshot.bucket_type == self.bucket_type
    }
}

impl Drop for ActivityWatchRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown();
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
    selected_port: Arc<AtomicU16>,
    api_key: String,
    profile: WatcherProfilePaths,
    connector_gateway: Option<Arc<crate::connectors::ConnectorGateway>>,
    ready: Arc<AtomicBool>,
    shutdown_slot: Arc<Mutex<Option<rocket::Shutdown>>>,
    startup_sender: mpsc::SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("No se pudo crear el runtime local: {error}"))?;
    runtime.block_on(async move {
        let mut last_error = None;
        for port in PORT_START..=PORT_END {
            if let Err(error) = write_watcher_server_config(&profile, port, &api_key) {
                let _ = startup_sender.send(Err(error.clone()));
                return Err(error);
            }
            let mut config = aw_server::config::AWConfig::default();
            config.address = "127.0.0.1".into();
            config.port = port;
            config.testing = false;
            config.cors.clear();
            config.cors_regex.clear();
            if connector_gateway.is_some() {
                config
                    .cors_regex
                    .push(crate::connectors::CHROMIUM_EXTENSION_ORIGIN_REGEX.into());
            }
            config.auth.api_key = Some(api_key.clone());
            let state = aw_server::endpoints::ServerState {
                datastore: Mutex::new(datastore.clone()),
                asset_resolver: aw_server::endpoints::AssetResolver::new(None),
                device_id: "enso-time-local".into(),
            };
            let launched = Arc::new(AtomicBool::new(false));
            let launched_fairing = Arc::clone(&launched);
            let ready_fairing = Arc::clone(&ready);
            let port_fairing = Arc::clone(&selected_port);
            let startup_fairing = startup_sender.clone();
            let fairing = rocket::fairing::AdHoc::on_liftoff("Ensō server ready", move |_| {
                let launched = Arc::clone(&launched_fairing);
                let ready = Arc::clone(&ready_fairing);
                let selected_port = Arc::clone(&port_fairing);
                let startup = startup_fairing.clone();
                Box::pin(async move {
                    selected_port.store(port, Ordering::Release);
                    launched.store(true, Ordering::Release);
                    ready.store(true, Ordering::Release);
                    let _ = startup.send(Ok(()));
                })
            });
            let rocket = aw_server::endpoints::build_rocket(state, config);
            let rocket = if let Some(gateway) = connector_gateway.clone() {
                crate::connectors::mount_gateway(rocket, gateway)
            } else {
                rocket
            };
            let rocket = match rocket.attach(fairing).ignite().await {
                Ok(rocket) => rocket,
                Err(error) => {
                    last_error = Some(format!(
                        "No se pudo preparar el servicio ActivityWatch en {port}: {error}"
                    ));
                    continue;
                }
            };
            let shutdown = rocket.shutdown();
            *shutdown_slot
                .lock()
                .map_err(|_| "No se pudo preparar el apagado del servicio local")? = Some(shutdown);
            match rocket.launch().await {
                Ok(_) => return Ok(()),
                Err(error) if !launched.load(Ordering::Acquire) => {
                    last_error = Some(format!(
                        "No se pudo enlazar el servicio ActivityWatch en {port}: {error}"
                    ));
                    continue;
                }
                Err(error) => {
                    return Err(format!("El servicio ActivityWatch se detuvo: {error}"));
                }
            }
        }
        let message = last_error
            .unwrap_or_else(|| "No hay un puerto local enlazable entre 5610 y 5699".to_string());
        let _ = startup_sender.send(Err(message.clone()));
        Err(message)
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

fn prepare_activity_database(path: &Path) -> Result<(), String> {
    let connection = Connection::open(path)
        .map_err(|error| format!("No se pudo preparar la base de actividad: {error}"))?;
    connection
        .execute_batch("PRAGMA secure_delete=ON; PRAGMA journal_mode=WAL;")
        .map_err(|error| format!("No se pudo activar el borrado seguro: {error}"))
}

fn compact_activity_database(path: &Path) -> Result<(), String> {
    let connection = Connection::open(path)
        .map_err(|error| format!("No se pudo compactar la base de actividad: {error}"))?;
    connection
        .busy_timeout(StdDuration::from_secs(5))
        .map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "PRAGMA secure_delete=ON;
             PRAGMA wal_checkpoint(TRUNCATE);
             VACUUM;
             PRAGMA wal_checkpoint(TRUNCATE);
             PRAGMA journal_mode=DELETE;",
        )
        .map_err(|error| format!("Los datos se borraron, pero la compactación falló: {error}"))
}

fn checkpoint_and_remove_sqlite_sidecars(path: &Path) -> Result<(), String> {
    let connection = Connection::open(path)
        .map_err(|error| format!("No se pudo cerrar la base de actividad: {error}"))?;
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|error| format!("No se pudo cerrar el WAL de actividad: {error}"))?;
    drop(connection);
    for suffix in ["-wal", "-shm"] {
        let sidecar = sqlite_sidecar_path(path, suffix);
        if sidecar.exists() {
            std::fs::remove_file(&sidecar).map_err(|error| {
                format!(
                    "No se pudo eliminar el archivo temporal {}: {error}",
                    sidecar.display()
                )
            })?;
        }
    }
    Ok(())
}

fn sqlite_sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn set_private_directory_permissions(path: &Path) -> Result<(), String> {
    #[cfg(not(unix))]
    let _ = path;
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
    #[cfg(not(unix))]
    let _ = path;
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

fn walk_private_transient_directories(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut transient = Vec::new();
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
            let name = entry.file_name();
            if matches!(
                name.to_string_lossy().to_ascii_lowercase().as_str(),
                "queued" | "log" | "logs" | "diagnostics"
            ) {
                transient.push(path);
            } else {
                pending.push(path);
            }
        }
    }
    Ok(transient)
}

fn delete_private_transient_data(root: &Path) -> Result<(), String> {
    for entry in walk_private_transient_directories(root)? {
        std::fs::remove_dir_all(&entry).map_err(|error| {
            format!(
                "No se pudo borrar el artefacto privado {}: {error}",
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
    exit_code: Option<i32>,
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
            exit_code: None,
        };
    };
    let Some(watcher) = guard.as_mut() else {
        return WatcherInspection {
            status: if binary_exists { "stopped" } else { "missing" }.into(),
            issue: None,
            exit_code: None,
        };
    };
    match watcher.child.try_wait() {
        Ok(None) => WatcherInspection {
            status: "running".into(),
            issue: None,
            exit_code: None,
        },
        Ok(Some(status)) => {
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
                exit_code: status.code(),
            }
        }
        Err(_) => WatcherInspection {
            status: "error".into(),
            issue: Some(kind.unavailable_code()),
            exit_code: None,
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

struct ClassificationMatch {
    category: String,
    activity_label: Option<String>,
    confidence: f64,
    source: String,
    rule_id: Option<String>,
}

fn classify(
    app: &str,
    title: Option<&str>,
    rules: &[ClassificationRuleV1],
    legacy_settings_rules: &[AppRule],
) -> ClassificationMatch {
    let normalized_app = normalize_match_text(app);
    let normalized_title = title.map(normalize_match_text).unwrap_or_default();
    let searchable = format!("{normalized_app} {normalized_title}");

    for scope in ["app-title", "app", "legacy"] {
        if let Some(rule) = rules
            .iter()
            .filter(|rule| {
                if rule.scope != scope {
                    return false;
                }
                match scope {
                    "app-title" => {
                        normalize_match_text(&rule.app) == normalized_app
                            && rule.title_pattern.as_deref().is_some_and(|pattern| {
                                normalized_title.contains(&normalize_match_text(pattern))
                            })
                    }
                    "app" => normalize_match_text(&rule.app) == normalized_app,
                    "legacy" => searchable.contains(&normalize_match_text(&rule.app)),
                    _ => false,
                }
            })
            .max_by(|left, right| {
                let left_specificity = left
                    .title_pattern
                    .as_deref()
                    .map(normalize_match_text)
                    .map(|value| value.chars().count())
                    .unwrap_or_default();
                let right_specificity = right
                    .title_pattern
                    .as_deref()
                    .map(normalize_match_text)
                    .map(|value| value.chars().count())
                    .unwrap_or_default();
                left_specificity
                    .cmp(&right_specificity)
                    .then_with(|| left.updated_at.cmp(&right.updated_at))
                    .then_with(|| left.id.cmp(&right.id))
            })
        {
            return ClassificationMatch {
                category: rule.category.clone(),
                activity_label: rule.activity_label.clone(),
                confidence: 1.0,
                source: "rule".into(),
                rule_id: Some(rule.id.clone()),
            };
        }
    }

    if let Some(rule) = legacy_settings_rules
        .iter()
        .find(|rule| searchable.contains(&normalize_match_text(&rule.pattern)))
    {
        return ClassificationMatch {
            category: rule.category.clone(),
            activity_label: rule.activity_label.clone(),
            confidence: 1.0,
            source: "rule".into(),
            rule_id: Some(rule.id.clone()),
        };
    }

    let category = built_in_category(&normalized_app, &normalized_title);
    ClassificationMatch {
        category: category.into(),
        activity_label: None,
        confidence: if category == "unknown" { 0.25 } else { 0.7 },
        source: if category == "unknown" {
            "unknown"
        } else {
            "builtin"
        }
        .into(),
        rule_id: None,
    }
}

fn built_in_category(app: &str, title: &str) -> &'static str {
    if is_browser(app) {
        if contains_any(title, &["google meet", "meet.google", "zoom meeting"]) {
            return "meetings";
        }
        if contains_any(title, &["gmail", "outlook mail"]) {
            return "communication";
        }
        return "unknown";
    }
    if contains_any(app, &["zoom", "microsoft teams", "facetime"]) {
        "meetings"
    } else if contains_any(app, &["slack", "mail", "outlook", "whatsapp", "discord"]) {
        "communication"
    } else if contains_any(
        app,
        &[
            "visual studio code",
            "terminal",
            "xcode",
            "figma",
            "photoshop",
            "notion",
        ],
    ) {
        "creation"
    } else if contains_any(app, &["kindle", "books", "coursera"]) {
        "learning"
    } else if contains_any(
        app,
        &[
            "system settings",
            "settings",
            "finder",
            "explorer",
            "excel",
            "sheets",
            "enso time",
        ],
    ) {
        "admin"
    } else {
        "unknown"
    }
}

pub fn normalize_match_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(
                    *character,
                    '\u{200b}'..='\u{200f}'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2060}'..='\u{206f}'
                        | '\u{feff}'
                )
        })
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn is_browser(normalized_app: &str) -> bool {
    [
        "google chrome",
        "chrome",
        "safari",
        "firefox",
        "microsoft edge",
        "arc",
        "brave browser",
        "orion",
    ]
    .iter()
    .any(|candidate| normalized_app == *candidate)
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

fn source_block_id(block_id: &str) -> &str {
    block_id
        .split_once('@')
        .map_or(block_id, |(source, _)| source)
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
    use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
    use tempfile::TempDir;

    #[test]
    fn retries_private_bind_without_touching_official_port() {
        // An actual ActivityWatch/Ensō process may already own one of these ports
        // on a developer machine. Reserve every candidate that is currently free;
        // candidates we cannot reserve are already exercising the same bind retry.
        let _official = TcpListener::bind((Ipv4Addr::LOCALHOST, 5600)).ok();
        let reservations: Vec<_> = (PORT_START..=PORT_START + 2)
            .filter_map(|port| {
                TcpListener::bind((Ipv4Addr::LOCALHOST, port))
                    .ok()
                    .map(|listener| (port, listener))
            })
            .collect();
        assert!(!reservations.is_empty());
        let temp = TempDir::new().unwrap();
        let runtime = ActivityWatchRuntime::start(
            &temp.path().join("data"),
            temp.path().join("empty-resources"),
        )
        .unwrap();
        let port = wait_until_ready(&runtime);
        assert!((PORT_START..=PORT_END).contains(&port));
        assert!(!reservations
            .iter()
            .any(|(reserved_port, _)| *reserved_port == port));
        assert_ne!(port, 5600);
    }

    #[test]
    fn connector_gateway_origin_does_not_abort_server_startup() {
        let temp = TempDir::new().unwrap();
        let store =
            Arc::new(crate::storage::LocalStore::open(&temp.path().join("enso.sqlite")).unwrap());
        let gate = Arc::new(crate::commands::CaptureGate::new());
        let gateway = Arc::new(crate::connectors::ConnectorGateway::new(store, gate));
        let runtime = ActivityWatchRuntime::start_with_gateway(
            &temp.path().join("data"),
            temp.path().join("empty-resources"),
            gateway,
        )
        .unwrap();

        assert!((PORT_START..=PORT_END).contains(&wait_until_ready(&runtime)));
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
    fn activity_database_uses_secure_delete_and_truncates_sidecars() {
        let temp = TempDir::new().unwrap();
        let database = temp.path().join("activity.sqlite");
        prepare_activity_database(&database).unwrap();
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch("PRAGMA secure_delete=ON;")
            .unwrap();
        let enabled: i64 = connection
            .query_row("PRAGMA secure_delete", [], |row| row.get(0))
            .unwrap();
        assert_eq!(enabled, 1);
        connection
            .execute_batch(
                "CREATE TABLE secret(value TEXT);
                 INSERT INTO secret VALUES('sensitive-marker');
                 DELETE FROM secret;",
            )
            .unwrap();
        drop(connection);
        compact_activity_database(&database).unwrap();
        checkpoint_and_remove_sqlite_sidecars(&database).unwrap();
        assert!(!sqlite_sidecar_path(&database, "-wal").exists());
        assert!(!sqlite_sidecar_path(&database, "-shm").exists());
        let bytes = std::fs::read(database).unwrap();
        assert!(!bytes
            .windows(b"sensitive-marker".len())
            .any(|window| window == b"sensitive-marker"));
    }

    #[test]
    fn private_cleanup_removes_queues_and_logs_only_inside_profile() {
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
        let logs = profile.home.join("Library/Logs/activitywatch");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(logs.join("watcher.log"), "private log").unwrap();
        let unrelated = profile.root.join("keep/config.toml");
        std::fs::create_dir_all(unrelated.parent().unwrap()).unwrap();
        std::fs::write(&unrelated, "keep").unwrap();

        delete_private_transient_data(&profile.root).unwrap();

        assert!(!legacy_queue.exists());
        assert!(!profiled_queue.exists());
        assert!(!logs.exists());
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
    fn timeline_splits_partial_afk_and_human_context_wins() {
        let temp = TempDir::new().unwrap();
        let runtime = ActivityWatchRuntime::start(
            &temp.path().join("data"),
            temp.path().join("empty-resources"),
        )
        .unwrap();
        let datastore = runtime.datastore().unwrap();
        let start = DateTime::parse_from_rfc3339("2026-08-21T12:00:00Z")
            .unwrap()
            .to_utc();
        for (id, bucket_type, client) in [
            ("timeline-window", "currentwindow", "aw-watcher-window"),
            ("timeline-afk", "afkstatus", "aw-watcher-afk"),
        ] {
            datastore
                .create_bucket(&aw_models::Bucket {
                    bid: None,
                    id: id.into(),
                    _type: bucket_type.into(),
                    client: client.into(),
                    hostname: "test".into(),
                    created: Some(start),
                    data: Map::new(),
                    metadata: aw_models::BucketMetadata::default(),
                    events: None,
                    last_updated: None,
                })
                .unwrap();
        }
        let mut window_data = Map::new();
        window_data.insert("app".into(), serde_json::Value::String("Terminal".into()));
        window_data.insert(
            "title".into(),
            serde_json::Value::String("Ensō tests".into()),
        );
        datastore
            .insert_events(
                "timeline-window",
                &[Event::new(start, Duration::minutes(10), window_data)],
            )
            .unwrap();
        let mut afk_data = Map::new();
        afk_data.insert("status".into(), serde_json::Value::String("afk".into()));
        datastore
            .insert_events(
                "timeline-afk",
                &[
                    Event::new(
                        start + Duration::minutes(4),
                        Duration::minutes(2),
                        afk_data.clone(),
                    ),
                    Event::new(start + Duration::minutes(8), Duration::zero(), afk_data),
                ],
            )
            .unwrap();
        let checkin = CheckInV1 {
            id: "human-context".into(),
            interval_start: (start + Duration::minutes(5)).to_rfc3339(),
            interval_end: (start + Duration::minutes(7)).to_rfc3339(),
            reason: "afk".into(),
            status: "answered".into(),
            activity_label: Some("Reunión caminando".into()),
            category: Some("meetings".into()),
            project: None,
            repetitive: Some(false),
            rules_clear: Some(false),
            created_at: start.to_rfc3339(),
            answered_at: Some((start + Duration::minutes(7)).to_rfc3339()),
        };
        let mut settings = SettingsV1::default();
        settings.capture_window_titles = true;
        let timeline = runtime
            .timeline(
                start,
                start + Duration::minutes(10),
                &settings,
                std::slice::from_ref(&checkin),
                &[],
                &[],
            )
            .unwrap();
        assert_eq!(timeline.len(), 5);
        assert_eq!(
            timeline
                .iter()
                .map(|block| (block.duration_seconds, block.category.as_str(), block.afk))
                .collect::<Vec<_>>(),
            vec![
                (240, "creation", false),
                (60, "break", true),
                (60, "meetings", true),
                (60, "meetings", false),
                (180, "creation", false),
            ]
        );
        assert!(timeline.iter().all(|block| block.correctable));

        let target = timeline
            .iter()
            .find(|block| block.category == "meetings" && block.afk)
            .unwrap();
        let override_item = ClassificationOverrideV1 {
            block_id: target.id.clone(),
            event_start: target.start.clone(),
            event_end: target.end.clone(),
            category: "focus".into(),
            activity_label: Some("Lectura en papel".into()),
            created_at: start.to_rfc3339(),
            updated_at: start.to_rfc3339(),
        };
        let base_override = ClassificationOverrideV1 {
            block_id: target.id.split_once('@').unwrap().0.into(),
            event_start: start.to_rfc3339(),
            event_end: (start + Duration::minutes(10)).to_rfc3339(),
            category: "personal".into(),
            activity_label: Some("Corrección anterior".into()),
            created_at: start.to_rfc3339(),
            updated_at: (start + Duration::minutes(1)).to_rfc3339(),
        };
        let corrected = runtime
            .timeline(
                start,
                start + Duration::minutes(10),
                &settings,
                &[checkin],
                &[],
                &[base_override, override_item],
            )
            .unwrap();
        let corrected = corrected
            .iter()
            .find(|block| block.start == target.start && block.end == target.end)
            .unwrap();
        assert_eq!(corrected.category, "focus");
        assert_eq!(corrected.classification_source, "override");
        assert!(corrected.afk);
    }

    #[test]
    fn explicit_rules_win_over_defaults() {
        let rules = vec![AppRule {
            id: "one".into(),
            pattern: "notion".into(),
            category: "focus".into(),
            activity_label: None,
        }];
        let classified = classify("Notion", None, &[], &rules);
        assert_eq!(classified.category, "focus");
        assert_eq!(classified.confidence, 1.0);
    }

    #[test]
    fn title_rules_are_more_specific_and_matching_ignores_invisible_unicode() {
        let now = "2026-08-22T12:00:00Z".to_string();
        let rules = vec![
            ClassificationRuleV1 {
                id: "app-rule".into(),
                scope: "app".into(),
                app: "Google Chrome".into(),
                title_pattern: None,
                category: "personal".into(),
                activity_label: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            },
            ClassificationRuleV1 {
                id: "title-rule".into(),
                scope: "app-title".into(),
                app: "\u{200e}Google Chrome".into(),
                title_pattern: Some("Client Portal".into()),
                category: "focus".into(),
                activity_label: Some("Trabajo de cliente".into()),
                created_at: now.clone(),
                updated_at: now,
            },
        ];
        let classified = classify(
            "Google Chrome",
            Some("Overview — CLIENT   PORTAL"),
            &rules,
            &[],
        );
        assert_eq!(classified.category, "focus");
        assert_eq!(classified.rule_id.as_deref(), Some("title-rule"));
        assert_eq!(
            classified.activity_label.as_deref(),
            Some("Trabajo de cliente")
        );
    }

    #[test]
    fn multipurpose_browsers_stay_unknown_without_a_useful_title_signal() {
        let classified = classify("Google Chrome", None, &[], &[]);
        assert_eq!(classified.category, "unknown");
        assert_eq!(classified.source, "unknown");
        let meeting = classify("Google Chrome", Some("Weekly sync — Google Meet"), &[], &[]);
        assert_eq!(meeting.category, "meetings");
    }

    #[test]
    fn watcher_retry_uses_bounded_exponential_delays_and_resets() {
        let mut retry = WatcherRetryState::new();
        assert!(retry.due());
        for expected in [30, 60, 120, 300, 300] {
            retry.failure();
            let delay = retry.retry_delay().unwrap().as_secs();
            assert!(
                (expected - 1..=expected).contains(&delay),
                "expected about {expected}s, got {delay}s"
            );
            assert!(!retry.due());
        }
        retry.success();
        assert!(retry.due());
        assert_eq!(retry.failures, 0);
        assert!(retry.next_retry.is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn missing_accessibility_stays_healthy_in_app_only_mode_without_restart_or_prompt() {
        let temp = TempDir::new().unwrap();
        let resources = compiled_watcher_resources(temp.path(), "permission_missing");
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        wait_until_ready(&runtime);

        runtime.start_watchers(true).unwrap();
        let initial_pids = runtime.test_managed_watcher_pids();
        assert!(initial_pids.0.is_some() && initial_pids.1.is_some());
        assert_eq!(
            runtime.capture_issue(),
            Some(CaptureIssueCode::AccessibilityPermissionRequired)
        );
        assert_eq!(
            runtime.watcher_statuses_with_timeout(StdDuration::from_millis(150)),
            ("running".into(), "running".into())
        );
        runtime.retry_watchers(true).unwrap();
        assert_eq!(runtime.test_managed_watcher_pids(), initial_pids);
        assert!(!temp.path().join("permission-requested").exists());

        let window_bucket = runtime.watcher_bucket_id(WatcherKind::Window);
        let events = runtime
            .datastore()
            .unwrap()
            .get_events(&window_bucket, None, None, None)
            .unwrap();
        assert!(!events.is_empty());
        assert!(events.iter().all(|event| !event.data.contains_key("url")));
        assert!(events.iter().all(|event| {
            event.data.get("title").and_then(|value| value.as_str()) == Some("excluded")
        }));

        let mut settings = SettingsV1::default();
        settings.capture_window_titles = true;
        let from = DateTime::parse_from_rfc3339("2026-08-21T11:59:00Z")
            .unwrap()
            .to_utc();
        let to = DateTime::parse_from_rfc3339("2026-08-21T12:02:00Z")
            .unwrap()
            .to_utc();
        let timeline = runtime
            .timeline(from, to, &settings, &[], &[], &[])
            .unwrap();
        assert!(!timeline.is_empty());
        assert!(timeline.iter().all(|block| block.title.is_none()));
        runtime.stop_watchers().unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn granting_accessibility_restarts_only_window_watcher_and_restores_titles() {
        let temp = TempDir::new().unwrap();
        let resources = compiled_watcher_resources(temp.path(), "permission_toggle");
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        wait_until_ready(&runtime);
        runtime.start_watchers(true).unwrap();
        let before = runtime.test_managed_watcher_pids();
        assert_eq!(
            runtime.capture_issue(),
            Some(CaptureIssueCode::AccessibilityPermissionRequired)
        );

        std::fs::write(temp.path().join("permission-granted"), "granted").unwrap();
        runtime.retry_watchers(true).unwrap();
        let after = runtime.test_managed_watcher_pids();
        assert_ne!(after.0, before.0);
        assert_eq!(after.1, before.1);
        assert_eq!(runtime.capture_issue(), None);

        let events = runtime
            .datastore()
            .unwrap()
            .get_events(
                &runtime.watcher_bucket_id(WatcherKind::Window),
                None,
                None,
                None,
            )
            .unwrap();
        assert!(events.iter().any(|event| {
            event.data.get("title").and_then(|value| value.as_str())
                == Some("enso-runtime-sensitive-marker")
        }));

        std::fs::remove_file(temp.path().join("permission-granted")).unwrap();
        std::thread::sleep(StdDuration::from_millis(200));
        assert_eq!(
            runtime.watcher_statuses_with_timeout(StdDuration::from_secs(5)),
            ("stopped".into(), "running".into())
        );
        assert_eq!(
            runtime.capture_issue(),
            Some(CaptureIssueCode::AccessibilityPermissionRequired)
        );
        runtime.start_watchers_if_due(true).unwrap();
        let degraded = runtime.test_managed_watcher_pids();
        assert_ne!(degraded.0, after.0);
        assert_eq!(degraded.1, after.1);
        assert_eq!(
            runtime.watcher_statuses_with_timeout(StdDuration::from_secs(5)),
            ("running".into(), "running".into())
        );
        runtime.stop_watchers().unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn accessibility_prompt_is_only_triggered_by_the_explicit_action() {
        let temp = TempDir::new().unwrap();
        let resources = compiled_watcher_resources(temp.path(), "permission_missing");
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        wait_until_ready(&runtime);
        runtime.start_watchers(true).unwrap();
        let marker = temp.path().join("permission-requested");
        assert!(!marker.exists());
        runtime.request_accessibility_permission().unwrap();
        assert_eq!(std::fs::read_to_string(marker).unwrap(), "requested");
        runtime.stop_watchers().unwrap();
    }

    #[test]
    fn watcher_lifecycle_is_enso_owned_and_server_port_is_released() {
        let temp = TempDir::new().unwrap();
        let resources = compiled_watcher_resources(temp.path(), "healthy");
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        let port = wait_until_ready(&runtime);

        runtime.start_watchers(true).unwrap();
        assert_eq!(
            runtime.watcher_statuses(),
            ("running".into(), "running".into())
        );
        std::thread::sleep(StdDuration::from_millis(250));
        assert_eq!(
            runtime.watcher_statuses_with_timeout(StdDuration::from_millis(150)),
            ("running".into(), "running".into())
        );
        assert_eq!(runtime.capture_issue(), None);
        assert!(runtime
            .bucket_snapshot(&runtime.watcher_bucket_id(WatcherKind::Window))
            .unwrap()
            .is_some());
        assert!(runtime
            .bucket_snapshot(&runtime.watcher_bucket_id(WatcherKind::Afk))
            .unwrap()
            .is_some());
        let window_bucket = runtime.watcher_bucket_id(WatcherKind::Window);
        let events = runtime
            .datastore()
            .unwrap()
            .get_events(&window_bucket, None, None, None)
            .unwrap();
        assert_eq!(
            events[0].data.get("title").and_then(|value| value.as_str()),
            Some("enso-runtime-sensitive-marker")
        );
        runtime.stop_watchers().unwrap();
        assert_eq!(
            runtime.watcher_statuses(),
            ("stopped".into(), "stopped".into())
        );
        runtime.delete_all_activity().unwrap();
        assert!(runtime
            .datastore()
            .unwrap()
            .get_buckets()
            .unwrap()
            .is_empty());
        for path in [
            runtime.database_path.clone(),
            sqlite_sidecar_path(&runtime.database_path, "-wal"),
            sqlite_sidecar_path(&runtime.database_path, "-shm"),
        ] {
            if path.is_file() {
                let bytes = std::fs::read(path).unwrap();
                assert!(!bytes
                    .windows(b"enso-runtime-sensitive-marker".len())
                    .any(|window| window == b"enso-runtime-sensitive-marker"));
            }
        }
        assert!(!sqlite_sidecar_path(&runtime.database_path, "-wal").exists());
        assert!(!sqlite_sidecar_path(&runtime.database_path, "-shm").exists());
        runtime.start_watchers(false).unwrap();
        assert_eq!(
            runtime.watcher_statuses(),
            ("running".into(), "running".into())
        );
        runtime.stop_watchers().unwrap();
        let private_queue = runtime.profile.data.join("activitywatch/aw-client/queued");
        let private_logs = runtime.profile.home.join("Library/Logs/activitywatch");
        std::fs::create_dir_all(&private_queue).unwrap();
        std::fs::create_dir_all(&private_logs).unwrap();
        std::fs::write(private_queue.join("queued.db"), "event").unwrap();
        std::fs::write(private_logs.join("watcher.log"), "log").unwrap();
        runtime.delete_queued_activity().unwrap();
        assert!(!private_queue.exists());
        assert!(!private_logs.exists());
        assert!(runtime.profile.diagnostics.exists());
        let database_path = runtime.database_path.clone();
        runtime.shutdown().unwrap();
        runtime.shutdown().unwrap();
        assert_eq!(runtime.port_if_ready(), None);
        assert!(runtime.start_watchers(false).is_err());

        TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .expect("explicit shutdown must release the private Rocket listener");
        assert!(!sqlite_sidecar_path(&database_path, "-wal").exists());
        assert!(!sqlite_sidecar_path(&database_path, "-shm").exists());
    }

    #[test]
    fn live_but_disconnected_watchers_degrade_and_both_are_stopped() {
        let temp = TempDir::new().unwrap();
        let resources = compiled_watcher_resources(temp.path(), "disconnected");
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        wait_until_ready(&runtime);

        let error = runtime
            .start_watchers_with_timeout(false, StdDuration::from_millis(500))
            .unwrap_err();
        assert!(error.contains("siguió vivo"));
        assert_eq!(
            runtime.capture_issue(),
            Some(CaptureIssueCode::WindowWatcherUnavailable)
        );
        assert_eq!(
            runtime.watcher_statuses(),
            ("stopped".into(), "stopped".into())
        );
    }

    #[test]
    fn shutdown_interrupts_heartbeat_wait_and_releases_runtime_promptly() {
        let temp = TempDir::new().unwrap();
        let resources = compiled_watcher_resources(temp.path(), "disconnected");
        let runtime =
            Arc::new(ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap());
        let port = wait_until_ready(&runtime);
        let starter_runtime = Arc::clone(&runtime);
        let starter = std::thread::spawn(move || starter_runtime.start_watchers(false));
        std::thread::sleep(StdDuration::from_millis(150));

        let shutdown_started = Instant::now();
        runtime.shutdown().unwrap();
        let elapsed = shutdown_started.elapsed();
        let start_error = starter.join().unwrap().unwrap_err();

        assert_eq!(start_error, CAPTURE_START_CANCELLED);
        assert!(elapsed < StdDuration::from_secs(2), "elapsed: {elapsed:?}");
        assert!(runtime.test_watchers_are_stopped());
        assert_eq!(runtime.port_if_ready(), None);
        assert_server_stopped(port);
        assert!(!sqlite_sidecar_path(&runtime.database_path, "-wal").exists());
        assert!(!sqlite_sidecar_path(&runtime.database_path, "-shm").exists());
    }

    #[test]
    fn shutdown_cancels_watcher_start_waiting_for_lifecycle_lock() {
        let temp = TempDir::new().unwrap();
        let runtime = Arc::new(
            ActivityWatchRuntime::start(
                &temp.path().join("data"),
                temp.path().join("empty-resources"),
            )
            .unwrap(),
        );
        wait_until_ready(&runtime);
        let lifecycle = runtime.watcher_lifecycle.lock().unwrap();
        let starter_runtime = Arc::clone(&runtime);
        let (lock_attempted, lock_attempted_rx) = mpsc::channel();
        runtime.signal_watcher_lifecycle_lock_attempt(lock_attempted);
        let (starter_result, starter_result_rx) = mpsc::channel();
        let starter = std::thread::spawn(move || {
            let result = starter_runtime.start_watchers(false);
            let _ = starter_result.send(result);
        });
        lock_attempted_rx
            .recv_timeout(StdDuration::from_secs(1))
            .unwrap();
        let shutdown_runtime = Arc::clone(&runtime);
        let (shutdown_result, shutdown_result_rx) = mpsc::channel();
        let shutdown = std::thread::spawn(move || {
            let result = shutdown_runtime.shutdown();
            let _ = shutdown_result.send(result);
        });

        for _ in 0..100 {
            if runtime.shutdown_requested() {
                break;
            }
            std::thread::sleep(StdDuration::from_millis(10));
        }
        assert!(runtime.shutdown_requested());
        drop(lifecycle);

        let starter_result = starter_result_rx
            .recv_timeout(StdDuration::from_secs(1))
            .unwrap();
        let shutdown_result = shutdown_result_rx
            .recv_timeout(StdDuration::from_secs(5))
            .unwrap();
        starter.join().unwrap();
        shutdown.join().unwrap();
        assert_eq!(starter_result.unwrap_err(), CAPTURE_START_CANCELLED);
        shutdown_result.unwrap();
    }

    #[test]
    fn healthy_then_disconnected_watchers_become_stale_and_can_retry() {
        let temp = TempDir::new().unwrap();
        let resources = compiled_watcher_resources(temp.path(), "healthy_then_disconnected");
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        wait_until_ready(&runtime);
        runtime.start_watchers(false).unwrap();
        assert_eq!(
            runtime.watcher_statuses(),
            ("running".into(), "running".into())
        );

        std::thread::sleep(StdDuration::from_millis(250));
        assert!(watcher_exit_status(&runtime.window_watcher)
            .unwrap()
            .is_none());
        assert!(watcher_exit_status(&runtime.afk_watcher).unwrap().is_none());
        assert_eq!(
            runtime.watcher_statuses_with_timeout(StdDuration::from_millis(150)),
            ("stopped".into(), "stopped".into())
        );
        assert_eq!(
            runtime.capture_issue(),
            Some(CaptureIssueCode::WindowWatcherUnavailable)
        );

        runtime.start_watchers(false).unwrap();
        assert_eq!(
            runtime.watcher_statuses(),
            ("running".into(), "running".into())
        );
        assert_eq!(runtime.capture_issue(), None);
    }

    #[test]
    fn retention_prune_compacts_and_restores_running_watchers() {
        let temp = TempDir::new().unwrap();
        let resources = compiled_watcher_resources(temp.path(), "healthy");
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        wait_until_ready(&runtime);
        runtime.start_watchers(false).unwrap();

        let deleted = runtime.prune_activity(Some(0)).unwrap();

        assert!(deleted >= 2);
        assert_eq!(
            runtime.watcher_statuses(),
            ("running".into(), "running".into())
        );
        assert_eq!(runtime.capture_issue(), None);
    }

    #[test]
    fn preexisting_exact_buckets_require_a_post_spawn_heartbeat() {
        let temp = TempDir::new().unwrap();
        let resources = compiled_watcher_resources(temp.path(), "healthy");
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        wait_until_ready(&runtime);
        for kind in [WatcherKind::Window, WatcherKind::Afk] {
            runtime
                .datastore()
                .unwrap()
                .create_bucket(&Bucket {
                    bid: None,
                    id: runtime.watcher_bucket_id(kind),
                    _type: kind.bucket_type().into(),
                    client: kind.binary_name().into(),
                    hostname: gethostname::gethostname().to_string_lossy().into_owned(),
                    created: None,
                    data: Map::new(),
                    metadata: BucketMetadata::default(),
                    events: None,
                    last_updated: None,
                })
                .unwrap();
        }
        let before = runtime.watcher_baseline(WatcherKind::Window).unwrap();

        runtime.start_watchers(false).unwrap();

        assert!(runtime.watcher_progressed(&before).unwrap());
        assert_eq!(runtime.capture_issue(), None);
    }

    #[test]
    fn early_single_instance_exit_is_reported_as_conflict_and_can_retry() {
        let temp = TempDir::new().unwrap();
        let resources = compiled_watcher_resources(temp.path(), "conflict");
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        wait_until_ready(&runtime);

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

    #[test]
    fn missing_afk_watcher_stops_only_enso_capture_and_reports_exact_issue() {
        let temp = TempDir::new().unwrap();
        let resources = compiled_watcher_resources(temp.path(), "healthy");
        std::fs::remove_file(fixture_binary_path(&resources, "aw-watcher-afk")).unwrap();
        let runtime = ActivityWatchRuntime::start(&temp.path().join("data"), resources).unwrap();
        wait_until_ready(&runtime);

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

    fn compiled_watcher_resources(root: &Path, mode: &str) -> PathBuf {
        let resources = root.join("resources");
        let source = root.join(format!("watcher-fixture-{mode}.rs"));
        let compiled = root.join(format!(
            "watcher-fixture-{mode}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let fixture_source = format!(
            r###"
use std::env;
use std::io::{{Read, Write}};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

fn request(port: u16, method: &str, path: &str, body: &str, key: &str) {{
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "{{method}} {{path}} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {{key}}\r\nContent-Type: application/json\r\nContent-Length: {{}}\r\nConnection: close\r\n\r\n{{body}}",
        body.as_bytes().len()
    ).unwrap();
    stream.flush().unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    assert!(
        response.starts_with(b"HTTP/1.1 2") || response.starts_with(b"HTTP/1.1 3"),
        "{{}}",
        String::from_utf8_lossy(&response)
    );
}}

fn main() {{
    let mode = {mode:?};
    let args: Vec<String> = env::args().collect();
    let permission_granted = std::path::Path::new({permission_granted:?});
    let permission_requested = std::path::Path::new({permission_requested:?});
    if args.get(1).map(String::as_str) == Some("--accessibility-status") {{
        if mode == "permission_missing" || (mode == "permission_toggle" && !permission_granted.exists()) {{
            std::process::exit(77);
        }}
        std::process::exit(0);
    }}
    if args.get(1).map(String::as_str) == Some("--request-accessibility") {{
        std::fs::write(permission_requested, "requested").unwrap();
        std::process::exit(77);
    }}
    if mode == "conflict" {{
        eprintln!("Another instance is already running, quitting.");
        std::process::exit(255);
    }}
    if mode == "healthy" || mode == "healthy_then_disconnected" || mode == "permission_missing" || mode == "permission_toggle" {{
        let port = env::var("ENSO_TEST_PORT").unwrap().parse::<u16>().unwrap();
        let bucket = env::var("ENSO_TEST_BUCKET_ID").unwrap();
        let bucket_type = env::var("ENSO_TEST_BUCKET_TYPE").unwrap();
        let client = env::var("ENSO_TEST_CLIENT_NAME").unwrap();
        let hostname = env::var("ENSO_TEST_HOSTNAME").unwrap();
        let key = env::var("ENSO_TEST_API_KEY").unwrap();
        // Only the macOS capture helper receives the key through the environment.
        if cfg!(target_os = "macos") && client == "aw-watcher-window" {{
            assert_eq!(env::var("ENSO_ACTIVITYWATCH_API_KEY").unwrap(), key);
            assert!(!args.iter().any(|argument| argument.contains(&key)));
        }}
        let body = format!(r#"{{{{"client":"{{}}","hostname":"{{}}","type":"{{}}"}}}}"#, client, hostname, bucket_type);
        request(port, "POST", &format!("/api/0/buckets/{{}}", bucket), &body, &key);
        let mut pulse = 0_u64;
        loop {{
            let window_mode = env::var("ENSO_TEST_WINDOW_MODE").unwrap_or_default();
            if client == "aw-watcher-window" && mode == "permission_toggle" && window_mode == "full" && !permission_granted.exists() {{
                std::process::exit(77);
            }}
            let title = if window_mode.starts_with("app-only") {{ "excluded" }} else {{ "enso-runtime-sensitive-marker" }};
            let event = format!(r#"{{{{"timestamp":"2026-08-21T12:00:00Z","duration":1.0,"data":{{{{"app":"fixture","title":"{{}}","status":"not-afk","process":{{}},"pulse":{{}}}}}}}}}}"#, title, std::process::id(), pulse);
            request(
                port,
                "POST",
                &format!("/api/0/buckets/{{}}/heartbeat?pulsetime=1", bucket),
                &event,
                &key
            );
            pulse += 1;
            if mode == "healthy_then_disconnected" {{ break; }}
            thread::sleep(Duration::from_millis(100));
        }}
    }}
    loop {{ thread::sleep(Duration::from_secs(30)); }}
}}
"###,
            permission_granted = root.join("permission-granted"),
            permission_requested = root.join("permission-requested"),
        );
        std::fs::write(&source, fixture_source).unwrap();
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = Command::new(rustc)
            .arg("--edition=2021")
            .arg(&source)
            .arg("-o")
            .arg(&compiled)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fixture compilation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        for name in ["aw-watcher-window", "aw-watcher-afk"] {
            let directory = resources.join("activitywatch").join(name);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::copy(&compiled, fixture_binary_path(&resources, name)).unwrap();
        }
        resources
    }

    fn fixture_binary_path(resources: &Path, name: &str) -> PathBuf {
        let directory = resources.join("activitywatch").join(name);
        #[cfg(target_os = "macos")]
        let binary = directory.join(match name {
            "aw-watcher-window" => WINDOW_HELPER_BINARY,
            "aw-watcher-afk" => AFK_HELPER_BINARY,
            _ => name,
        });
        #[cfg(target_os = "windows")]
        let binary = directory.join(format!("{name}.exe"));
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        let binary = directory.join(name);
        binary
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

    fn assert_server_stopped(port: u16) {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        assert!(
            TcpStream::connect_timeout(&address, StdDuration::from_millis(50)).is_err(),
            "private server still accepts connections on {address}"
        );
    }
}
