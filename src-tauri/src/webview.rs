use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl};

/// Manages child webviews within the main window
pub struct WebviewManager {
    webviews: HashMap<String, tauri::Webview>,
}

impl WebviewManager {
    pub fn new() -> Self {
        Self {
            webviews: HashMap::new(),
        }
    }

    pub fn add(&mut self, id: String, webview: tauri::Webview) {
        self.webviews.insert(id, webview);
    }

    pub fn remove(&mut self, id: &str) -> Option<tauri::Webview> {
        self.webviews.remove(id)
    }

    pub fn get(&self, id: &str) -> Option<&tauri::Webview> {
        self.webviews.get(id)
    }
}

pub struct WebviewState {
    pub manager: Mutex<WebviewManager>,
}

/// Create a child webview in the main window
#[tauri::command]
pub async fn create_webview(
    app: AppHandle,
    state: tauri::State<'_, WebviewState>,
    id: String,
    url: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;

    // Get the underlying window (not the webview window)
    let window_ref = window.as_ref().window();

    // Get app data directory for persistent webview storage (cookies, cache, etc.)
    let data_dir = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .join("webview_data");

    // Create the child webview with a standard browser user agent and persistent storage
    let webview = window_ref
        .add_child(
            tauri::webview::WebviewBuilder::new(
                &id,
                WebviewUrl::External(url.parse().map_err(|e| format!("Invalid URL: {}", e))?),
            )
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .data_directory(data_dir),
            LogicalPosition::new(x, y),
            LogicalSize::new(width, height),
        )
        .map_err(|e| format!("Failed to create webview: {}", e))?;

    // Store the webview reference
    let mut manager = state.manager.lock().unwrap();
    manager.add(id.clone(), webview);

    tracing::info!("Created child webview: {} at ({}, {}) {}x{}", id, x, y, width, height);
    Ok(())
}

/// Update position and size of a child webview
#[tauri::command]
pub async fn update_webview(
    state: tauri::State<'_, WebviewState>,
    id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let manager = state.manager.lock().unwrap();
    let webview = manager.get(&id).ok_or("Webview not found")?;

    webview
        .set_position(LogicalPosition::new(x, y))
        .map_err(|e| format!("Failed to set position: {}", e))?;

    webview
        .set_size(LogicalSize::new(width, height))
        .map_err(|e| format!("Failed to set size: {}", e))?;

    Ok(())
}

/// Close and remove a child webview
#[tauri::command]
pub async fn close_webview(
    state: tauri::State<'_, WebviewState>,
    id: String,
) -> Result<(), String> {
    let mut manager = state.manager.lock().unwrap();

    if let Some(webview) = manager.remove(&id) {
        webview
            .close()
            .map_err(|e| format!("Failed to close webview: {}", e))?;
        tracing::info!("Closed child webview: {}", id);
    }

    Ok(())
}

/// Navigate a webview to a new URL
#[tauri::command]
pub async fn navigate_webview(
    state: tauri::State<'_, WebviewState>,
    id: String,
    url: String,
) -> Result<(), String> {
    let manager = state.manager.lock().unwrap();
    let webview = manager.get(&id).ok_or("Webview not found")?;

    webview
        .eval(&format!("window.location.href = {}", serde_json::json!(url)))
        .map_err(|e| format!("Failed to navigate: {}", e))?;

    Ok(())
}

/// Execute JavaScript in a webview
#[tauri::command]
pub async fn eval_webview(
    state: tauri::State<'_, WebviewState>,
    id: String,
    script: String,
) -> Result<(), String> {
    let manager = state.manager.lock().unwrap();
    let webview = manager.get(&id).ok_or("Webview not found")?;

    webview
        .eval(&script)
        .map_err(|e| format!("Failed to eval: {}", e))?;

    Ok(())
}
