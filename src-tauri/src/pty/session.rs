use chrono::{DateTime, Utc};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{error, info};
use uuid::Uuid;

/// Represents an active PTY session
pub struct PtySession {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub created_at: DateTime<Utc>,
    pub master: Arc<std::sync::Mutex<Box<dyn MasterPty + Send>>>,
    pub writer: Arc<std::sync::Mutex<Box<dyn Write + Send>>>,
    pub child: Arc<std::sync::Mutex<Box<dyn Child + Send + Sync>>>,
    pub cols: u16,
    pub rows: u16,
}

/// Session info for frontend (serializable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtySessionInfo {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub created_at: String,
    pub cols: u16,
    pub rows: u16,
    pub is_alive: bool,
}

/// Request to create a new PTY session
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSessionRequest {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

/// Terminal output event payload
#[derive(Debug, Clone, Serialize)]
pub struct TerminalOutput {
    pub session_id: String,
    pub data: Vec<u8>,
}

/// Terminal exit event payload
#[derive(Debug, Clone, Serialize)]
pub struct TerminalExit {
    pub session_id: String,
    pub exit_code: Option<u32>,
}

/// Manages multiple PTY sessions
pub struct PtySessionManager {
    sessions: HashMap<String, PtySession>,
}

impl PtySessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Spawn a new PTY session
    pub fn spawn_session(
        &mut self,
        app_handle: AppHandle,
        request: CreateSessionRequest,
    ) -> Result<PtySessionInfo, String> {
        let id = Uuid::new_v4().to_string();
        let cols = request.cols.unwrap_or(80);
        let rows = request.rows.unwrap_or(24);

        // Default to user's shell from SHELL env var, fallback to /bin/sh
        let command = request.command.unwrap_or_else(|| {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
        });
        let args = request.args.unwrap_or_default();

        info!("Spawning PTY session: {} {} {:?}", id, command, args);

        // Get the PTY system
        let pty_system = native_pty_system();

        // Create PTY pair
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        // Build command
        let mut cmd = CommandBuilder::new(&command);
        for arg in &args {
            cmd.arg(arg);
        }

        // Set working directory
        if let Some(ref cwd) = request.cwd {
            let expanded = shellexpand::tilde(cwd).to_string();
            cmd.cwd(&expanded);
        } else if let Some(home) = dirs::home_dir() {
            cmd.cwd(home);
        }

        // Set TERM for color support
        cmd.env("TERM", "xterm-256color");

        // Spawn the child process
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn command: {}", e))?;

        // Get reader from master for output streaming
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone reader: {}", e))?;

        // Get writer for input
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take writer: {}", e))?;

        let session_id = id.clone();
        let app_handle_clone = app_handle.clone();

        // Spawn output reader thread
        std::thread::spawn(move || {
            Self::read_output(session_id, reader, app_handle_clone);
        });

        let session = PtySession {
            id: id.clone(),
            command: command.clone(),
            args: args.clone(),
            cwd: request.cwd.clone(),
            created_at: Utc::now(),
            master: Arc::new(std::sync::Mutex::new(pair.master)),
            writer: Arc::new(std::sync::Mutex::new(writer)),
            child: Arc::new(std::sync::Mutex::new(child)),
            cols,
            rows,
        };

        let info = self.session_to_info(&session);
        self.sessions.insert(id, session);

        Ok(info)
    }

    /// Read output from PTY and emit events
    fn read_output(
        session_id: String,
        mut reader: Box<dyn Read + Send>,
        app_handle: AppHandle,
    ) {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    // EOF - process exited
                    info!("PTY session {} EOF", session_id);
                    break;
                }
                Ok(n) => {
                    let output = TerminalOutput {
                        session_id: session_id.clone(),
                        data: buf[..n].to_vec(),
                    };
                    if let Err(e) = app_handle.emit("terminal-output", output) {
                        error!("Failed to emit terminal output: {}", e);
                    }
                }
                Err(e) => {
                    error!("Error reading PTY output: {}", e);
                    break;
                }
            }
        }

        // Emit exit event
        let exit = TerminalExit {
            session_id: session_id.clone(),
            exit_code: None,
        };
        if let Err(e) = app_handle.emit("terminal-exit", exit) {
            error!("Failed to emit terminal exit: {}", e);
        }
    }

    /// Write data to PTY stdin
    pub fn write_to_session(&self, session_id: &str, data: &[u8]) -> Result<(), String> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let mut writer = session
            .writer
            .lock()
            .map_err(|e| format!("Failed to lock writer: {}", e))?;

        writer
            .write_all(data)
            .map_err(|e| format!("Failed to write to PTY: {}", e))?;

        Ok(())
    }

    /// Resize PTY
    pub fn resize_session(&mut self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let master = session
            .master
            .lock()
            .map_err(|e| format!("Failed to lock master: {}", e))?;

        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to resize PTY: {}", e))?;

        session.cols = cols;
        session.rows = rows;

        Ok(())
    }

    /// Kill a session
    pub fn kill_session(&mut self, session_id: &str) -> Result<(), String> {
        let session = self
            .sessions
            .remove(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let mut child = session
            .child
            .lock()
            .map_err(|e| format!("Failed to lock child: {}", e))?;

        child
            .kill()
            .map_err(|e| format!("Failed to kill process: {}", e))?;

        info!("Killed PTY session: {}", session_id);
        Ok(())
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Vec<PtySessionInfo> {
        self.sessions
            .values()
            .map(|s| self.session_to_info(s))
            .collect()
    }

    /// Get a specific session
    pub fn get_session(&self, session_id: &str) -> Option<PtySessionInfo> {
        self.sessions.get(session_id).map(|s| self.session_to_info(s))
    }

    /// Convert session to info struct
    fn session_to_info(&self, session: &PtySession) -> PtySessionInfo {
        let is_alive = session
            .child
            .lock()
            .map(|mut c| c.try_wait().ok().flatten().is_none())
            .unwrap_or(false);

        PtySessionInfo {
            id: session.id.clone(),
            command: session.command.clone(),
            args: session.args.clone(),
            cwd: session.cwd.clone(),
            created_at: session.created_at.to_rfc3339(),
            cols: session.cols,
            rows: session.rows,
            is_alive,
        }
    }
}

impl Default for PtySessionManager {
    fn default() -> Self {
        Self::new()
    }
}
