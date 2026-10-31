use serde::Serialize;
use tauri::State;
use veil_core::AppResult;

use crate::app_state::AppState;

/// Where Veil keeps profiles, the database, the keystore and extensions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataLocationInfo {
    pub path: String,
    /// True when this is the OS-default location rather than a user choice.
    pub is_default: bool,
    /// A move recorded for the next startup, if any. The UI uses this to say a restart is needed.
    pub pending_path: Option<String>,
}

impl DataLocationInfo {
    fn read(state: &AppState) -> Self {
        let paths = state.paths.lock().expect("paths lock poisoned");
        Self {
            path: state.data_dir.to_string_lossy().into_owned(),
            is_default: state.data_dir == state.default_data_dir,
            pending_path: paths
                .pending_data_dir
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
        }
    }
}

#[tauri::command]
pub fn get_data_location(state: State<'_, AppState>) -> DataLocationInfo {
    DataLocationInfo::read(&state)
}

/// Record a data-directory move, to be performed at the next startup.
///
/// The move deliberately does not happen here — see `AppState::request_data_dir_move`. The command
/// returns the new pending state so the UI can ask for a restart instead of pretending it is done.
#[tauri::command]
pub fn set_data_location(state: State<'_, AppState>, path: String) -> AppResult<DataLocationInfo> {
    state.request_data_dir_move(std::path::PathBuf::from(path.trim()))?;
    Ok(DataLocationInfo::read(&state))
}

/// Drop a recorded move that has not happened yet.
#[tauri::command]
pub fn cancel_data_location_move(state: State<'_, AppState>) -> AppResult<DataLocationInfo> {
    state.cancel_data_dir_move()?;
    Ok(DataLocationInfo::read(&state))
}

/// Restart the app so a pending data-directory move is applied.
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) {
    app.restart();
}
