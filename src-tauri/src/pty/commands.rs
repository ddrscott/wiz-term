use super::session::{CreateSessionRequest, PtySessionInfo};
use crate::storage::database::TerminalPreferences;
use crate::AppState;
use chrono::Utc;
use std::io::Write;
use base64::{engine::general_purpose::STANDARD, Engine};

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

/// Save image data to a temp file and return the path.
/// Used for pasting/dropping images into the terminal for Claude Code.
#[tauri::command]
pub async fn save_temp_image(
    data: String, // base64 encoded image data
    filename: Option<String>,
) -> Result<String, String> {
    // Determine file extension from data URL or filename
    let (extension, image_data) = if data.starts_with("data:image/") {
        // Parse data URL: data:image/png;base64,<data>
        let parts: Vec<&str> = data.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err("Invalid data URL format".to_string());
        }

        let mime_ext = if parts[0].contains("png") {
            "png"
        } else if parts[0].contains("jpeg") || parts[0].contains("jpg") {
            "jpg"
        } else if parts[0].contains("gif") {
            "gif"
        } else if parts[0].contains("webp") {
            "webp"
        } else {
            "png" // default
        };

        (mime_ext.to_string(), parts[1].to_string())
    } else {
        // Raw base64 - guess from filename or default to png
        let ext = filename
            .as_ref()
            .and_then(|f| f.rsplit('.').next())
            .unwrap_or("png")
            .to_string();
        (ext, data)
    };

    // Decode base64
    let bytes = STANDARD
        .decode(&image_data)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    // Create temp file with original filename or generated name
    let temp_dir = std::env::temp_dir();
    let file_name = filename.unwrap_or_else(|| {
        format!("wizterm-image-{}.{}", uuid::Uuid::new_v4(), extension)
    });
    let file_path = temp_dir.join(&file_name);

    // Write to file
    let mut file = std::fs::File::create(&file_path)
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    file.write_all(&bytes)
        .map_err(|e| format!("Failed to write image data: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}
