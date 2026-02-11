use super::session::{CreateSessionRequest, PtySessionInfo};
use crate::storage::database::TerminalPreferences;
use crate::AppState;
use chrono::Utc;

#[tauri::command]
pub async fn pty_create_session(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    request: CreateSessionRequest,
) -> Result<PtySessionInfo, String> {
    let mut manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    let session_info = manager.spawn_session(app, request)?;

    // Save to database
    state
        .db
        .save_terminal_session(
            &session_info.id,
            &session_info.command,
            &session_info.args,
            session_info.cwd.as_deref(),
            Utc::now().timestamp(),
        )
        .map_err(|e| format!("Failed to save session to database: {}", e))?;

    Ok(session_info)
}

#[tauri::command]
pub async fn pty_write(
    state: tauri::State<'_, AppState>,
    session_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    manager.write_to_session(&session_id, &data)
}

#[tauri::command]
pub async fn pty_resize(
    state: tauri::State<'_, AppState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let mut manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    manager.resize_session(&session_id, cols, rows)
}

#[tauri::command]
pub async fn pty_kill(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let mut manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    manager.kill_session(&session_id)?;

    // Update database to mark session as ended
    state
        .db
        .update_terminal_session_end(&session_id, None)
        .map_err(|e| format!("Failed to update session in database: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn pty_list_sessions(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PtySessionInfo>, String> {
    let manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    Ok(manager.list_sessions())
}

#[tauri::command]
pub async fn pty_get_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<Option<PtySessionInfo>, String> {
    let manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    Ok(manager.get_session(&session_id))
}

#[tauri::command]
pub async fn pty_save_layout(
    state: tauri::State<'_, AppState>,
    layout_json: String,
) -> Result<(), String> {
    state
        .db
        .save_terminal_layout(&layout_json)
        .map_err(|e| format!("Failed to save layout: {}", e))
}

#[tauri::command]
pub async fn pty_get_layout(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    state
        .db
        .get_terminal_layout()
        .map_err(|e| format!("Failed to get layout: {}", e))
}

#[tauri::command]
pub async fn pty_save_preferences(
    state: tauri::State<'_, AppState>,
    preferences: TerminalPreferences,
) -> Result<(), String> {
    state
        .db
        .save_terminal_preferences(&preferences)
        .map_err(|e| format!("Failed to save preferences: {}", e))
}

#[tauri::command]
pub async fn pty_get_preferences(
    state: tauri::State<'_, AppState>,
) -> Result<TerminalPreferences, String> {
    state
        .db
        .get_terminal_preferences()
        .map_err(|e| format!("Failed to get preferences: {}", e))
}
