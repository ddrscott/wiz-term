use std::sync::Mutex;
use tauri::menu::{Menu, MenuItemBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager};

pub mod pty;
mod storage;

use pty::PtySessionManager;
use storage::database::Database;

pub struct AppState {
    pub db: Database,
    pub pty_manager: Mutex<PtySessionManager>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter("wiz_term=debug,info")
        .init();

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init());

    // Add MCP bridge plugin for testing (debug builds only)
    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    builder
        .setup(|app| {
            // Build App menu (macOS standard menu with Quit, Hide, etc.)
            let app_menu = SubmenuBuilder::new(app, "wiz-term")
                .about(None)
                .separator()
                .services()
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
                .build()?;

            // Build View menu
            let toggle_minimap = MenuItemBuilder::with_id("toggle_minimap", "Toggle Minimap")
                .accelerator("CmdOrCtrl+Shift+M")
                .build(app)?;
            let pin_minimap = MenuItemBuilder::with_id("pin_minimap", "Pin to Top")
                .build(app)?;
            let reset_minimap = MenuItemBuilder::with_id("reset_minimap", "Reset Position")
                .build(app)?;

            let view_menu = SubmenuBuilder::new(app, "View")
                .item(&toggle_minimap)
                .item(&pin_minimap)
                .separator()
                .item(&reset_minimap)
                .build()?;

            // Build Edit menu with standard items
            let edit_menu = SubmenuBuilder::new(app, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;

            // Build Window menu
            // Note: .close_window() removed so frontend Cmd+W handler closes splits instead
            let window_menu = SubmenuBuilder::new(app, "Window")
                .minimize()
                .maximize()
                .build()?;

            let menu = Menu::with_items(app, &[&app_menu, &edit_menu, &view_menu, &window_menu])?;
            app.set_menu(menu)?;

            let db = Database::new().expect("Failed to initialize database");
            db.run_migrations().expect("Failed to run migrations");

            // Clean up terminal sessions from previous runs
            if let Ok(marked) = db.mark_all_terminal_sessions_ended() {
                if marked > 0 {
                    tracing::info!("Marked {} stale terminal sessions as ended", marked);
                }
            }
            if let Ok(deleted) = db.cleanup_old_terminal_sessions(7) {
                if deleted > 0 {
                    tracing::info!("Cleaned up {} old terminal sessions", deleted);
                }
            }

            app.manage(AppState {
                db,
                pty_manager: Mutex::new(PtySessionManager::new()),
            });

            tracing::info!("wiz-term app initialized");
            app.emit("backend-ready", ()).unwrap();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pty::pty_create_session,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_kill,
            pty::pty_list_sessions,
            pty::pty_get_session,
            pty::pty_save_layout,
            pty::pty_get_layout,
            pty::pty_save_preferences,
            pty::pty_get_preferences,
            pty::save_temp_image,
        ])
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            match id {
                "toggle_minimap" => {
                    let _ = app.emit("menu-toggle-minimap", ());
                }
                "pin_minimap" => {
                    let _ = app.emit("menu-pin-minimap", ());
                }
                "reset_minimap" => {
                    let _ = app.emit("menu-reset-minimap", ());
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
