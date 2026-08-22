mod activitywatch;
mod commands;
mod diagnostics;
mod models;
mod storage;

use activitywatch::ActivityWatchRuntime;
use commands::AppState;
use std::sync::Arc;
use storage::LocalStore;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, RunEvent};
use tauri_plugin_autostart::MacosLauncher;

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main_window(app);
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .macos_launcher(MacosLauncher::LaunchAgent)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("No se pudo resolver el directorio de datos: {error}"))?;
            std::fs::create_dir_all(&app_data_dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&app_data_dir, std::fs::Permissions::from_mode(0o700))?;
            }
            let store = Arc::new(LocalStore::open(&app_data_dir.join("enso-time.sqlite"))?);
            let packaged_resources = app.path().resource_dir().map_err(|error| {
                format!("No se pudo resolver el directorio de recursos: {error}")
            })?;
            let development_resources =
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
            let resource_dir = resolve_resource_dir(packaged_resources, development_resources);
            let config_dir = app.path().config_dir().map_err(|error| {
                format!("No se pudo resolver el directorio de configuración: {error}")
            })?;
            let local_data_dir = app.path().local_data_dir().map_err(|error| {
                format!("No se pudo resolver el directorio local del capturador: {error}")
            })?;
            let activity = Arc::new(ActivityWatchRuntime::start(
                &app_data_dir.join("activitywatch"),
                resource_dir,
                &config_dir,
                &local_data_dir,
            )?);
            let state = AppState {
                store: Arc::clone(&store),
                activity: Arc::clone(&activity),
                app_data_dir,
            };
            app.manage(state);
            install_tray(app)?;

            let settings = store.settings()?;
            if settings.onboarding_completed && settings.autostart {
                use tauri_plugin_autostart::ManagerExt;
                if let Err(error) = app.autolaunch().enable() {
                    eprintln!("Ensō Time autostart: {error}");
                }
            }
            // The server starts in a dedicated thread; watchers retry briefly while
            // Rocket binds its loopback socket.
            let pause_is_active = store
                .paused_until()?
                .and_then(|value| models::parse_utc(&value).ok())
                .is_some_and(|until| until > chrono::Utc::now());
            if settings.onboarding_completed && !pause_is_active {
                let watcher_activity = Arc::clone(&activity);
                std::thread::Builder::new()
                    .name("enso-watcher-bootstrap".into())
                    .spawn(move || {
                        for _ in 0..20 {
                            if watcher_activity.port_if_ready().is_some() {
                                if let Err(error) =
                                    watcher_activity.start_watchers(settings.capture_window_titles)
                                {
                                    eprintln!("Ensō Time watchers: {error}");
                                }
                                return;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(150));
                        }
                        eprintln!("Ensō Time: el servicio local no respondió a tiempo");
                    })?;
            }
            commands::start_scheduler(app.handle().clone(), store, activity);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::get_tracking_status,
            commands::get_timeline,
            commands::list_checkins,
            commands::answer_checkin,
            commands::get_study_progress,
            commands::generate_diagnostic,
            commands::pause_tracking,
            commands::resume_tracking,
            commands::export_data,
            commands::delete_all_data,
            commands::check_for_update,
            commands::open_external,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Ensō Time");

    app.run(|app, event| {
        if let RunEvent::ExitRequested { .. } | RunEvent::Exit = event {
            if let Some(state) = app.try_state::<AppState>() {
                let _ = state.activity.stop_watchers();
            }
        }
    });
}

fn resolve_resource_dir(
    packaged: std::path::PathBuf,
    development: std::path::PathBuf,
) -> std::path::PathBuf {
    // Tauri preserves the configured `resources/` prefix in production bundles.
    // Older layouts placed its contents directly at the resource root, so accept both.
    [packaged.join("resources"), packaged, development.clone()]
        .into_iter()
        .find(|candidate| candidate.join("activitywatch").exists())
        .unwrap_or(development)
}

fn install_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Abrir Ensō Time", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pausar 30 minutos", true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", "Reanudar", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &pause, &resume, &quit])?;
    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Ensō Time")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "pause" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let _ = state.activity.stop_watchers();
                    let until = chrono::Utc::now() + chrono::Duration::minutes(30);
                    let _ = state.store.set_paused_until(Some(&until.to_rfc3339()));
                }
            }
            "resume" => {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(settings) = state.store.settings() {
                        if settings.onboarding_completed
                            && state
                                .activity
                                .start_watchers(settings.capture_window_titles)
                                .is_ok()
                        {
                            let _ = state.store.set_paused_until(None);
                        }
                    }
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
