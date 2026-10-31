//! Veil desktop backend entry point.
//!
//! Wires application state and the Tauri command surface. Commands are thin: they validate
//! input and delegate to services, which compose the workspace crates. The webview can only
//! reach the backend through these commands (see `capabilities/default.json`).

mod app_state;
mod commands;
mod default_page;
mod engine_root;
mod local_api;
mod migrate;
mod paths;
mod services;

use tauri::Manager;

use app_state::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_local_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("Veil");
            let state = std::sync::Arc::new(AppState::open(data_dir).unwrap_or_else(|err| {
                // First-run keystore / database failure is fatal: refusing to start beats
                // silently running on an unencrypted fallback.
                eprintln!("veil: fatal startup error: {err}");
                std::process::exit(1);
            }));
            // Startup outcome, recorded independently of the database.
            state.diagnostics().record_ok(
                crate::services::DiagnosticComponent::Startup,
                crate::services::DiagnosticAction::AppStarted,
            );
            // The local API listener holds only a weak handle to `AppState` (upgraded per
            // request), so the strong `Arc` here can be unwrapped and managed by value for
            // the `State<'_, AppState>` command surface. Reconcile now so an already-enabled
            // API starts listening on launch.
            let server = local_api::LocalApiServer::new(std::sync::Arc::downgrade(&state));
            server.reconcile(&state);
            app.manage(server);
            // The server holds only a weak handle, so the strong `Arc` unwraps here and the
            // value is managed for the `State<'_, AppState>` command surface.
            let state = match std::sync::Arc::try_unwrap(state) {
                Ok(state) => state,
                Err(_) => {
                    eprintln!("veil: internal error: local api server leaked a strong reference");
                    std::process::exit(1);
                }
            };
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Runtime / engine
            commands::get_onboarding_status,
            commands::complete_onboarding,
            commands::reset_onboarding,
            commands::save_recovery_phrase_txt,
            commands::generate_recovery_phrase,
            commands::verify_sensitive_action,
            commands::begin_totp_enrollment,
            commands::confirm_totp_enrollment,
            commands::disable_totp,
            commands::get_vault_status,
            commands::unlock_vault,
            commands::recover_vault_with_recovery_phrase,
            // AI / automation
            commands::get_ai_settings,
            commands::update_ai_settings,
            commands::has_ai_provider_credentials,
            commands::list_ai_model_configs,
            commands::upsert_ai_model_config,
            commands::list_ai_audit_logs,
            commands::export_ai_audit_logs,
            commands::list_ai_tasks,
            commands::get_ai_task,
            commands::list_ai_task_events,
            commands::decide_ai_task_approval,
            commands::run_ai_task,
            commands::pause_ai_task_for_human_verification,
            commands::list_ai_profile_facts,
            commands::upsert_ai_profile_fact,
            commands::get_ai_session,
            // Local API
            commands::get_local_api_settings,
            commands::update_local_api_settings,
            commands::list_local_api_keys,
            commands::generate_local_api_key,
            commands::rotate_local_api_key,
            // Diagnostics
            commands::preview_diagnostics,
            commands::export_diagnostics,
            // Extensions / plugins
            commands::list_extensions,
            commands::get_extension,
            commands::import_extension_from_store,
            commands::import_extension_from_local,
            commands::update_extension,
            commands::delete_extension,
            // Runtime / engine
            commands::get_runtime_state,
            commands::get_runtime_install_root,
            commands::set_runtime_install_root,
            commands::get_data_location,
            commands::set_data_location,
            commands::cancel_data_location_move,
            commands::restart_app,
            commands::list_installed_runtimes,
            commands::list_available_runtimes,
            commands::install_runtime_version,
            commands::install_runtime_from_local_zip,
            commands::cancel_runtime_install,
            commands::set_current_runtime,
            commands::uninstall_runtime_version,
            // Profiles
            commands::list_profiles,
            commands::get_profile,
            commands::create_profile,
            commands::update_profile,
            commands::delete_profile,
            commands::list_running_profiles,
            commands::is_profile_running,
            commands::export_profiles,
            commands::import_profiles,
            commands::randomize_fingerprint,
            commands::optimize_fingerprint_for_proxy,
            commands::validate_fingerprint,
            commands::update_profile_fingerprint,
            commands::delete_profile_data,
            commands::clear_profile_cache,
            commands::get_profile_cookies,
            commands::update_profile_cookies,
            commands::list_profile_credentials,
            commands::create_profile_credential,
            commands::update_profile_credential,
            commands::delete_profile_credential,
            commands::launch_profile,
            commands::launch_temporary_profile,
            commands::launch_custom_temporary_profile,
            commands::close_temporary_profile,
            commands::launch_profile_for_ai,
            commands::stop_profile,
            // Groups
            commands::list_groups,
            commands::create_group,
            commands::update_group,
            commands::delete_group,
            // Proxies
            commands::list_proxies,
            commands::get_proxy,
            commands::create_proxy,
            commands::update_proxy,
            commands::delete_proxy,
            commands::reveal_proxy_credentials,
            commands::batch_set_profile_proxy,
            commands::test_proxy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Veil");
}
