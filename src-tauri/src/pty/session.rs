use super::tmux;
use chrono::{DateTime, Utc};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};
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
    /// Whether this session is backed by tmux (persistent)
    pub is_tmux: bool,
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
    /// Whether this session is backed by tmux (persistent across app restarts)
    pub is_tmux: bool,
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
    /// Whether tmux is available on this system
    tmux_available: bool,
    /// Whether to use tmux for session persistence (can be disabled)
    use_tmux: bool,
}

impl PtySessionManager {
    pub fn new() -> Self {
        let tmux_available = tmux::is_tmux_available();
        if tmux_available {
            if let Some(version) = tmux::get_tmux_version() {
                info!("tmux detected: {}", version);
            }
        } else {
            info!("tmux not available, sessions will not persist across app restarts");
        }

        Self {
            sessions: HashMap::new(),
            tmux_available,
            use_tmux: tmux_available, // Enable by default if available
        }
    }

    /// Check if tmux is being used for persistence
    pub fn is_using_tmux(&self) -> bool {
        self.tmux_available && self.use_tmux
    }

    /// Enable or disable tmux usage
    pub fn set_use_tmux(&mut self, use_tmux: bool) {
        self.use_tmux = use_tmux && self.tmux_available;
    }

    /// List existing tmux sessions that can be reconnected
    pub fn list_reconnectable_sessions(&self) -> Vec<tmux::TmuxSessionInfo> {
        if !self.is_using_tmux() {
            return Vec::new();
        }
        tmux::list_wizterm_sessions()
    }

    /// Reconnect to an existing tmux session
    pub fn reconnect_session(
        &mut self,
        app_handle: AppHandle,
        session_id: String,
        cols: u16,
        rows: u16,
    ) -> Result<PtySessionInfo, String> {
        if !self.is_using_tmux() {
            return Err("tmux is not available".to_string());
        }

        // Check if we already have an active PTY session for this ID
        // This prevents duplicate connections when frontend refreshes
        if let Some(existing) = self.sessions.get(&session_id) {
            info!("Session {} already has active PTY, returning existing", session_id);
            return Ok(self.session_to_info(existing));
        }

        // Check if tmux session exists
        if !tmux::session_exists(&session_id) {
            return Err(format!("tmux session {} not found", session_id));
        }

        info!("Reconnecting to tmux session: {}", session_id);

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

        // Build tmux attach command
        let (cmd_name, args) = tmux::get_attach_command(&session_id)
            .ok_or("tmux not found")?;
        let mut cmd = CommandBuilder::new(&cmd_name);
        for arg in &args {
            cmd.arg(arg);
        }

        // Set TERM for color support
        cmd.env("TERM", "xterm-256color");

        // Spawn the tmux attach process
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to attach to tmux session: {}", e))?;

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

        let session_id_clone = session_id.clone();
        let app_handle_clone = app_handle.clone();

        // Spawn output reader thread
        std::thread::spawn(move || {
            Self::read_output(session_id_clone, reader, app_handle_clone);
        });

        let session = PtySession {
            id: session_id.clone(),
            command: "tmux".to_string(),
            args: args.clone(),
            cwd: None,
            created_at: Utc::now(), // Note: actual creation time is in tmux
            master: Arc::new(std::sync::Mutex::new(pair.master)),
            writer: Arc::new(std::sync::Mutex::new(writer)),
            child: Arc::new(std::sync::Mutex::new(child)),
            cols,
            rows,
            is_tmux: true,
        };

        let info = self.session_to_info(&session);
        self.sessions.insert(session_id, session);

        Ok(info)
    }

    /// Spawn a new PTY session
    /// If tmux is available, creates a persistent tmux session and attaches to it
    pub fn spawn_session(
        &mut self,
        app_handle: AppHandle,
        request: CreateSessionRequest,
    ) -> Result<PtySessionInfo, String> {
        let id = Uuid::new_v4().to_string();
        let cols = request.cols.unwrap_or(80);
        let rows = request.rows.unwrap_or(24);

        // Try to use tmux if available
        if self.is_using_tmux() {
            match self.spawn_tmux_session(app_handle.clone(), id.clone(), request.clone(), cols, rows) {
                Ok(info) => return Ok(info),
                Err(e) => {
                    warn!("Failed to create tmux session, falling back to direct PTY: {}", e);
                    // Fall through to direct PTY
                }
            }
        }

        // Direct PTY (no tmux or tmux failed)
        self.spawn_direct_session(app_handle, id, request, cols, rows, false)
    }

    /// Spawn a session using tmux for persistence
    fn spawn_tmux_session(
        &mut self,
        app_handle: AppHandle,
        id: String,
        request: CreateSessionRequest,
        cols: u16,
        rows: u16,
    ) -> Result<PtySessionInfo, String> {
        // Create the tmux session first (detached)
        let cwd = request.cwd.as_deref();
        tmux::create_tmux_session(&id, cwd)?;

        info!("Created tmux session for: {}", id);

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

        // Build tmux attach command
        let (cmd_name, args) = tmux::get_attach_command(&id)
            .ok_or("tmux not found")?;
        let mut cmd = CommandBuilder::new(&cmd_name);
        for arg in &args {
            cmd.arg(arg);
        }

        // Set TERM for color support
        cmd.env("TERM", "xterm-256color");

        // Spawn the tmux attach process
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| {
                // Clean up tmux session if attach fails
                let _ = tmux::kill_tmux_session(&id);
                format!("Failed to attach to tmux session: {}", e)
            })?;

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
            command: "tmux".to_string(),
            args: args.clone(),
            cwd: request.cwd.clone(),
            created_at: Utc::now(),
            master: Arc::new(std::sync::Mutex::new(pair.master)),
            writer: Arc::new(std::sync::Mutex::new(writer)),
            child: Arc::new(std::sync::Mutex::new(child)),
            cols,
            rows,
            is_tmux: true,
        };

        let info = self.session_to_info(&session);
        self.sessions.insert(id, session);

        Ok(info)
    }

    /// Spawn a direct PTY session (no tmux)
    fn spawn_direct_session(
        &mut self,
        app_handle: AppHandle,
        id: String,
        request: CreateSessionRequest,
        cols: u16,
        rows: u16,
        is_tmux: bool,
    ) -> Result<PtySessionInfo, String> {
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
            is_tmux,
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
    /// If session is tmux-backed, also kills the tmux session
    pub fn kill_session(&mut self, session_id: &str) -> Result<(), String> {
        let session = self
            .sessions
            .remove(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let is_tmux = session.is_tmux;

        let mut child = session
            .child
            .lock()
            .map_err(|e| format!("Failed to lock child: {}", e))?;

        child
            .kill()
            .map_err(|e| format!("Failed to kill process: {}", e))?;

        // Also kill the tmux session if this was a tmux-backed session
        if is_tmux {
            if let Err(e) = tmux::kill_tmux_session(session_id) {
                warn!("Failed to kill tmux session {}: {}", session_id, e);
            }
        }

        info!("Killed PTY session: {}", session_id);
        Ok(())
    }

    /// List all sessions (filters out stale tmux sessions)
    pub fn list_sessions(&mut self) -> Vec<PtySessionInfo> {
        // Clean up stale tmux sessions first
        if self.is_using_tmux() {
            let reconnectable: std::collections::HashSet<String> = self
                .list_reconnectable_sessions()
                .into_iter()
                .map(|s| s.session_id)
                .collect();

            // Find sessions to remove (tmux sessions that no longer exist)
            let stale_ids: Vec<String> = self
                .sessions
                .iter()
                .filter(|(_, s)| s.is_tmux && !reconnectable.contains(&s.id))
                .map(|(id, _)| id.clone())
                .collect();

            // Remove stale sessions
            for id in stale_ids {
                info!("Removing stale tmux session from PTY manager: {}", id);
                self.sessions.remove(&id);
            }
        }

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
            is_tmux: session.is_tmux,
        }
    }
}

impl Default for PtySessionManager {
    fn default() -> Self {
        Self::new()
    }
}
