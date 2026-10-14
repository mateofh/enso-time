use crate::commands::AppState;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, State};

pub struct InstallerSmoke {
    pub root: PathBuf,
    report: PathBuf,
    completed: AtomicBool,
}

impl InstallerSmoke {
    pub fn from_args() -> Result<Option<Self>, String> {
        let args: Vec<_> = std::env::args().collect();
        if !args.iter().any(|arg| arg == "--smoke-test") {
            return Ok(None);
        }
        let report = args
            .iter()
            .position(|arg| arg == "--smoke-report")
            .and_then(|position| args.get(position + 1))
            .ok_or("Smoke testing requires --smoke-report PATH")?;
        let root =
            std::env::temp_dir().join(format!("enso-installer-smoke-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .map_err(|error| error.to_string())?;
        }
        Ok(Some(Self {
            root,
            report: report.into(),
            completed: AtomicBool::new(false),
        }))
    }
}

// The frontend calls this only after settings, timeline, study, intake, and
// diagnosis load successfully. Normal launches do not have InstallerSmoke state.
#[tauri::command]
pub fn complete_installer_smoke_test(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let Some(smoke) = app.try_state::<InstallerSmoke>() else {
        return Ok(());
    };
    if smoke.completed.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let result = (|| -> Result<(), String> {
        for _ in 0..100 {
            if state.activity.port_if_ready().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if state.activity.port_if_ready().is_none() {
            return Err("Packaged local server did not become ready".into());
        }
        if state.app_data_dir != smoke.root {
            return Err("Smoke data isolation failed".into());
        }
        let mut settings = state.store.settings()?;
        settings.role = "Installer smoke test".into();
        state.store.save_settings(&settings)?;
        if state.store.settings()?.role != "Installer smoke test" {
            return Err("SQLite roundtrip failed".into());
        }
        let report = json!({
            "status": "passed", "version": env!("CARGO_PKG_VERSION"),
            "sourceCommit": option_env!("ENSO_SOURCE_COMMIT"),
            "checks": ["webview-initialization", "tauri-ipc", "local-server", "sqlite-read-write"],
            "captureWithOsPermissions": "requires-manual-validation",
        });
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&smoke.report)
            .map_err(|error| error.to_string())?;
        file.write_all(report.to_string().as_bytes())
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        Ok(())
    })();
    if let Err(error) = &result {
        eprintln!("Ensō installer smoke: {error}");
    }
    app.exit(if result.is_ok() { 0 } else { 1 });
    result
}
