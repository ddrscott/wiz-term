use super::session::{CreateSessionRequest, PtySessionInfo};
use super::tmux::TmuxSessionInfo;
use crate::storage::database::TerminalPreferences;
use crate::AppState;
use chrono::Utc;
use serde::Serialize;

/// Info about a reconnectable session (for frontend)
#[derive(Debug, Clone, Serialize)]
pub struct ReconnectableSession {
    pub session_id: String,
    pub tmux_session_name: String,
    pub created_at: i64,
    pub attached: bool,
}

impl From<TmuxSessionInfo> for ReconnectableSession {
    fn from(info: TmuxSessionInfo) -> Self {
        Self {
            session_id: info.session_id,
            tmux_session_name: info.tmux_session_name,
            created_at: info.created_at,
            attached: info.attached,
        }
    }
}

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
    let mut manager = state
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

/// Check if tmux is being used for session persistence
#[tauri::command]
pub async fn pty_is_using_tmux(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    Ok(manager.is_using_tmux())
}

/// List existing tmux sessions that can be reconnected
#[tauri::command]
pub async fn pty_list_reconnectable(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ReconnectableSession>, String> {
    let manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    Ok(manager
        .list_reconnectable_sessions()
        .into_iter()
        .map(ReconnectableSession::from)
        .collect())
}

/// Reconnect to an existing tmux session
#[tauri::command]
pub async fn pty_reconnect_session(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    session_id: String,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<PtySessionInfo, String> {
    let mut manager = state
        .pty_manager
        .lock()
        .map_err(|e| format!("Failed to lock PTY manager: {}", e))?;

    let cols = cols.unwrap_or(80);
    let rows = rows.unwrap_or(24);

    manager.reconnect_session(app, session_id, cols, rows)
}

/// Get the tmux config file content
#[tauri::command]
pub async fn pty_get_tmux_config() -> Result<String, String> {
    super::tmux::read_config()
}

/// Set the tmux config file content
#[tauri::command]
pub async fn pty_set_tmux_config(content: String) -> Result<(), String> {
    super::tmux::write_config(&content)
}

/// Reset tmux config to defaults
#[tauri::command]
pub async fn pty_reset_tmux_config() -> Result<String, String> {
    super::tmux::reset_config_to_defaults()?;
    super::tmux::read_config()
}

/// Get the tmux config file path
#[tauri::command]
pub async fn pty_get_tmux_config_path() -> Result<String, String> {
    Ok(super::tmux::get_config_path().to_string_lossy().to_string())
}
