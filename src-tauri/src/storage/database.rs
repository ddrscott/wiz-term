use chrono::Utc;
use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new() -> SqliteResult<Self> {
        let db_path = Self::get_db_path();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn get_db_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wiz-term")
            .join("wiz-term.db")
    }

    pub fn run_migrations(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            -- Terminal sessions for persistence
            CREATE TABLE IF NOT EXISTS terminal_sessions (
                id TEXT PRIMARY KEY,
                command TEXT NOT NULL,
                args TEXT NOT NULL,
                cwd TEXT,
                created_at INTEGER NOT NULL,
                ended_at INTEGER,
                exit_code INTEGER
            );

            CREATE INDEX IF NOT EXISTS idx_terminal_sessions_created ON terminal_sessions(created_at DESC);

            -- Terminal layout for split pane persistence
            CREATE TABLE IF NOT EXISTS terminal_layout (
                id INTEGER PRIMARY KEY DEFAULT 1,
                layout_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );

            -- Terminal preferences/settings
            CREATE TABLE IF NOT EXISTS terminal_preferences (
                id INTEGER PRIMARY KEY DEFAULT 1,
                font_size INTEGER NOT NULL DEFAULT 13,
                font_family TEXT NOT NULL DEFAULT 'SF Mono',
                scrollback INTEGER NOT NULL DEFAULT 10000,
                cursor_blink INTEGER NOT NULL DEFAULT 1,
                minimap_refresh_ms INTEGER NOT NULL DEFAULT 200,
                use_webgl INTEGER NOT NULL DEFAULT 1,
                shell_path TEXT NOT NULL DEFAULT '/bin/zsh',
                updated_at INTEGER NOT NULL
            );
        "#,
        )?;

        // Migration: Add use_webgl column if missing (for existing databases)
        let _ = conn.execute(
            "ALTER TABLE terminal_preferences ADD COLUMN use_webgl INTEGER NOT NULL DEFAULT 1",
            [],
        );

        // Migration: Add shell_path column if missing
        let _ = conn.execute(
            "ALTER TABLE terminal_preferences ADD COLUMN shell_path TEXT NOT NULL DEFAULT '/bin/zsh'",
            [],
        );

        Ok(())
    }

    // ========== Terminal Session Methods ==========

    /// Save a new terminal session
    pub fn save_terminal_session(
        &self,
        id: &str,
        command: &str,
        args: &[String],
        cwd: Option<&str>,
        created_at: i64,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let args_json = serde_json::to_string(args).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            r#"
            INSERT OR REPLACE INTO terminal_sessions (id, command, args, cwd, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![id, command, args_json, cwd, created_at],
        )?;
        Ok(())
    }

    /// Update terminal session when it ends
    pub fn update_terminal_session_end(
        &self,
        id: &str,
        exit_code: Option<i32>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            r#"
            UPDATE terminal_sessions
            SET ended_at = ?2, exit_code = ?3
            WHERE id = ?1
            "#,
            params![id, now, exit_code],
        )?;
        Ok(())
    }

    /// Get recent terminal sessions (not ended)
    pub fn get_active_terminal_sessions(&self) -> SqliteResult<Vec<TerminalSessionRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, command, args, cwd, created_at, ended_at, exit_code
            FROM terminal_sessions
            WHERE ended_at IS NULL
            ORDER BY created_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let args_str: String = row.get(2)?;
            let args: Vec<String> = serde_json::from_str(&args_str).unwrap_or_default();
            Ok(TerminalSessionRecord {
                id: row.get(0)?,
                command: row.get(1)?,
                args,
                cwd: row.get(3)?,
                created_at: row.get(4)?,
                ended_at: row.get(5)?,
                exit_code: row.get(6)?,
            })
        })?;

        rows.collect()
    }

    /// Delete a terminal session record
    pub fn delete_terminal_session(&self, id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM terminal_sessions WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Clean up old ended sessions (keep last N days)
    pub fn cleanup_old_terminal_sessions(&self, days_to_keep: i64) -> SqliteResult<usize> {
        let conn = self.conn.lock().unwrap();
        let cutoff = Utc::now().timestamp() - (days_to_keep * 24 * 60 * 60);
        let deleted = conn.execute(
            "DELETE FROM terminal_sessions WHERE ended_at IS NOT NULL AND ended_at < ?1",
            [cutoff],
        )?;
        Ok(deleted)
    }

    /// Mark all active sessions as ended (called on app startup)
    pub fn mark_all_terminal_sessions_ended(&self) -> SqliteResult<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp();
        let updated = conn.execute(
            "UPDATE terminal_sessions SET ended_at = ?1 WHERE ended_at IS NULL",
            [now],
        )?;
        Ok(updated)
    }

    // ========== Terminal Layout Methods ==========

    /// Save terminal layout
    pub fn save_terminal_layout(&self, layout_json: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            r#"
            INSERT OR REPLACE INTO terminal_layout (id, layout_json, updated_at)
            VALUES (1, ?1, ?2)
            "#,
            params![layout_json, now],
        )?;
        Ok(())
    }

    /// Get terminal layout
    pub fn get_terminal_layout(&self) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result: Result<String, _> = conn.query_row(
            "SELECT layout_json FROM terminal_layout WHERE id = 1",
            [],
            |row| row.get(0),
        );
        Ok(result.ok())
    }

    // ========== Terminal Preferences Methods ==========

    /// Save terminal preferences
    pub fn save_terminal_preferences(&self, prefs: &TerminalPreferences) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp();
        conn.execute(
            r#"
            INSERT OR REPLACE INTO terminal_preferences (id, font_size, font_family, scrollback, cursor_blink, minimap_refresh_ms, use_webgl, shell_path, updated_at)
            VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![prefs.font_size, prefs.font_family, prefs.scrollback, prefs.cursor_blink as i32, prefs.minimap_refresh_ms, prefs.use_webgl as i32, prefs.shell_path, now],
        )?;
        Ok(())
    }

    /// Get terminal preferences
    pub fn get_terminal_preferences(&self) -> SqliteResult<TerminalPreferences> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT font_size, font_family, scrollback, cursor_blink, minimap_refresh_ms, use_webgl, shell_path FROM terminal_preferences WHERE id = 1",
            [],
            |row| {
                Ok(TerminalPreferences {
                    font_size: row.get(0)?,
                    font_family: row.get(1)?,
                    scrollback: row.get(2)?,
                    cursor_blink: row.get::<_, i32>(3)? != 0,
                    minimap_refresh_ms: row.get(4)?,
                    use_webgl: row.get::<_, i32>(5).unwrap_or(1) != 0,
                    shell_path: row.get::<_, String>(6).unwrap_or_else(|_| "/bin/zsh".to_string()),
                })
            },
        );

        // Return defaults if no preferences saved yet
        result.or_else(|_| Ok(TerminalPreferences::default()))
    }
}

/// Terminal preferences struct
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TerminalPreferences {
    pub font_size: i32,
    pub font_family: String,
    pub scrollback: i32,
    pub cursor_blink: bool,
    pub minimap_refresh_ms: i32,
    pub use_webgl: bool,
    pub shell_path: String,
}

impl Default for TerminalPreferences {
    fn default() -> Self {
        Self {
            font_size: 13,
            font_family: "SF Mono".to_string(),
            scrollback: 10000,
            cursor_blink: true,
            minimap_refresh_ms: 200,
            use_webgl: true,
            shell_path: "/bin/zsh".to_string(),
        }
    }
}

/// Record struct for terminal sessions from database
#[derive(Debug, Clone)]
pub struct TerminalSessionRecord {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub created_at: i64,
    pub ended_at: Option<i64>,
    pub exit_code: Option<i32>,
}
