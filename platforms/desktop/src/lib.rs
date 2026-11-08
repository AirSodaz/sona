#![recursion_limit = "512"]
//! Sona Desktop Native Library
//!
//! Milestone 1: Desktop Foundation, Core Linking & Tauri Excision

pub mod app;
mod assets;
pub mod event_bus;
pub mod integrations;
pub mod platform;
pub mod services;
pub mod theme;
pub mod views;

pub use app::bootstrap;
pub use event_bus::AppEventBus;
pub use services::DesktopServices;

/// Initializes and runs the Sona desktop application.
pub fn run() {
    if let Err(e) = run_app() {
        panic!("Error while running desktop application: {:?}", e);
    }
}

/// Sets up the runtime environment and starts the GPUI desktop application.
pub fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Ensure Tokio multi-threaded runtime is active
    let _tokio_rt = if tokio::runtime::Handle::try_current().is_err() {
        Some(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?,
        )
    } else {
        None
    };
    let _tokio_guard = _tokio_rt.as_ref().map(|rt| rt.enter());

    log::info!(
        "Sona Desktop starting | version={} | channel={} | commit={} | dirty={} | build_time={}",
        env!("SONA_DISPLAY_VERSION"),
        env!("SONA_BUILD_CHANNEL"),
        env!("SONA_COMMIT_SHA"),
        env!("SONA_IS_DIRTY"),
        env!("SONA_BUILD_TIMESTAMP")
    );

    // 3. Single-instance lock enforcement (Feature 6)
    let _instance_guard = match platform::single_instance::SingleInstanceGuard::acquire()? {
        Some(guard) => guard,
        None => {
            log::warn!(
                "Another instance of Sona is already running. Restoring window and exiting cleanly."
            );
            platform::single_instance::focus_existing_window("Sona");
            return Ok(());
        }
    };

    // 4. Initialize active data directory, SQLite WAL pool & database migration
    let data_roots = sona_runtime_fs::default_desktop_app_data_roots();
    let default_data_dir = data_roots
        .into_iter()
        .next()
        .unwrap_or_else(|| std::path::PathBuf::from("data"));

    let app_local_data_dir =
        platform::storage::location::resolve_active_data_dir(&default_data_dir);
    std::fs::create_dir_all(&app_local_data_dir)?;

    let db = platform::storage::database::open_and_migrate_sqlite_for_path_with_prompt(
        &app_local_data_dir,
        platform::startup_dialog::prompt_legacy_database_migration,
    )?;

    let sqlite_context = std::sync::Arc::new(sona_sqlite::SqliteApplicationContext::from_database(
        app_local_data_dir.clone(),
        db.clone(),
    )?);
    let sqlite_state = platform::database::DesktopSqliteState::new(sqlite_context);
    let sync_config_path = app_local_data_dir.join(platform::sync::SYNC_CONFIG_FILE);

    // 5. Initialize in-memory real-time event bus (Feature 5)
    let event_bus = std::sync::Arc::new(AppEventBus::new(1024));

    // 6. Assemble decoupled domain services container (DesktopServices)
    let services = std::sync::Arc::new(
        DesktopServices::builder()
            .sqlite(sqlite_state)
            .event_emitter(event_bus.clone())
            .sync_config_path(sync_config_path)
            .build()
            .map_err(|e| format!("Failed to build DesktopServices: {e}"))?,
    );

    // 7. Launch GPUI Kit application event loop
    let app_services = std::sync::Arc::clone(&services);
    let app_event_bus = std::sync::Arc::clone(&event_bus);

    gpui_kit::application()
        .with_assets(assets::AppAssets)
        .run(move |cx| {
            app::bootstrap(cx, app_services, app_event_bus);
        });

    log::info!("Sona Desktop shutting down cleanly.");
    Ok(())
}
