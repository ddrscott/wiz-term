use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use tracing::{debug, info, warn};

/// Cached path to tmux binary
static TMUX_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Find the tmux binary by querying the user's login shell for the full PATH
fn find_tmux() -> Option<PathBuf> {
    // First try current PATH (works in dev mode or if PATH is set)
    if let Ok(output) = Command::new("which").arg("tmux").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                let path = PathBuf::from(&path);
                if path.exists() {
                    debug!("Found tmux via PATH: {:?}", path);
                    return Some(path);
                }
            }
        }
    }

    // When launched from Finder, PATH is minimal and SHELL might not be set.
    // Try multiple approaches to find tmux.

    // Get user's shell - try SHELL env var, then query directory service, then default to zsh
    let shell = std::env::var("SHELL").ok().or_else(|| {
        // Query macOS directory service for user's shell
        Command::new("dscl")
            .args([".", "-read", &format!("/Users/{}", std::env::var("USER").unwrap_or_default()), "UserShell"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                String::from_utf8(o.stdout).ok().and_then(|s| {
                    s.split_whitespace().last().map(|s| s.to_string())
                })
            })
    }).unwrap_or_else(|| "/bin/zsh".to_string());

    debug!("Using shell for tmux lookup: {}", shell);

    // Use login shell (-l) to source user's profile (don't use -i as it causes issues with zsh)
    if let Ok(output) = Command::new(&shell)
        .args(["-l", "-c", "which tmux"])
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                let path = PathBuf::from(&path);
                if path.exists() {
                    info!("Found tmux via login shell: {:?}", path);
                    return Some(path);
                }
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            debug!("Login shell query failed: {}", stderr);
        }
    }

    // Last resort: check common Homebrew locations directly
    for path_str in &["/opt/homebrew/bin/tmux", "/usr/local/bin/tmux"] {
        let path = PathBuf::from(path_str);
        if path.exists() {
            info!("Found tmux at hardcoded path: {:?}", path);
            return Some(path);
        }
    }

    warn!("tmux not found via PATH, login shell, or common locations");
    None
}

/// Get the path to tmux binary (cached)
pub fn get_tmux_path() -> Option<&'static PathBuf> {
    TMUX_PATH.get_or_init(find_tmux).as_ref()
}

/// Create a Command for tmux with the correct path
fn tmux_command() -> Option<Command> {
    get_tmux_path().map(Command::new)
}

/// Session prefix for wiz-term tmux sessions
pub const TMUX_SESSION_PREFIX: &str = "wizterm-";

/// Dedicated socket name for wiz-term tmux sessions
/// Using a separate socket ensures our config is always applied
pub const TMUX_SOCKET_NAME: &str = "wizterm";

/// Default tmux configuration for transparent operation
/// This makes tmux invisible while preserving session persistence and scrollback
pub const DEFAULT_TMUX_CONFIG: &str = r#"# wiz-term - Transparent tmux config
# This makes tmux invisible while preserving session persistence
# Edit this file to customize tmux behavior

# === Visual Elements: OFF ===
set -g status off                    # Hide status bar
set -g pane-border-status off        # Hide pane borders
set -g visual-activity off           # No activity alerts
set -g visual-bell off               # No bell flash
set -g visual-silence off            # No silence alerts

# === Mouse: Pass-through to applications ===
set -g mouse on                      # Enable mouse support
# Mouse scroll goes to application (vim, etc)
# Use Shift+scroll to access tmux scrollback

# === Performance ===
set -g escape-time 10                # 10ms escape delay (vim-friendly)

# === Copy Mode: Minimal (xterm.js handles selection) ===
set -g mode-keys vi                  # vi keys if entering copy-mode

# === Terminal Compatibility ===
set -g default-terminal "xterm-256color"
set -ga terminal-overrides ",xterm-256color:Tc"  # True color support
set -gq allow-passthrough on         # Allow escape sequences to pass through (images, etc)

# === UTF-8 Support ===
set -gq utf8 on                      # Enable UTF-8 (older tmux)
set -gq mouse-utf8 on                # UTF-8 mouse input (older tmux)
setw -gq utf8 on                     # Window UTF-8 mode

# === Session Behavior ===
set -g detach-on-destroy on          # Detach client when session is destroyed (don't switch to other sessions)
set -g remain-on-exit off            # Clean exit behavior
set -g history-limit 50000           # Generous scrollback buffer
"#;

/// Get the path to the wiz-term tmux config file
pub fn get_config_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("wiz-term")
        .join("tmux.conf")
}

/// Ensure the tmux config file exists, creating with defaults if needed
pub fn ensure_config_exists() -> Result<PathBuf, String> {
    let path = get_config_path();

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    // Create config file with defaults if it doesn't exist
    if !path.exists() {
        std::fs::write(&path, DEFAULT_TMUX_CONFIG)
            .map_err(|e| format!("Failed to write tmux config: {}", e))?;
        info!("Created default tmux config at: {:?}", path);
    }

    Ok(path)
}

/// Reset tmux config to defaults
pub fn reset_config_to_defaults() -> Result<(), String> {
    let path = get_config_path();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    std::fs::write(&path, DEFAULT_TMUX_CONFIG)
        .map_err(|e| format!("Failed to write tmux config: {}", e))?;

    info!("Reset tmux config to defaults at: {:?}", path);
    Ok(())
}

/// Read the current tmux config content
pub fn read_config() -> Result<String, String> {
    let path = get_config_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read tmux config: {}", e))
    } else {
        Ok(DEFAULT_TMUX_CONFIG.to_string())
    }
}

/// Write custom tmux config content
pub fn write_config(content: &str) -> Result<(), String> {
    let path = get_config_path();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write tmux config: {}", e))?;

    info!("Updated tmux config at: {:?}", path);
    Ok(())
}

/// Check if tmux is installed and available
pub fn is_tmux_available() -> bool {
    get_tmux_path().is_some()
}

/// Get tmux version string
pub fn get_tmux_version() -> Option<String> {
    tmux_command()?
        .arg("-V")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
}

/// Create a new tmux session (detached)
/// Returns the session name on success
pub fn create_tmux_session(session_id: &str, cwd: Option<&str>) -> Result<String, String> {
    let session_name = format!("{}{}", TMUX_SESSION_PREFIX, session_id);

    // Ensure our transparent config exists
    let config_path = ensure_config_exists()?;

    // Get user's default shell
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    let mut cmd = tmux_command().ok_or("tmux not found")?;

    // Use dedicated socket and config file
    // The -L flag creates an isolated tmux server for wiz-term
    cmd.arg("-L").arg(TMUX_SOCKET_NAME);
    cmd.arg("-f").arg(&config_path);

    cmd.arg("new-session")
        .arg("-d")  // detached
        .arg("-s")
        .arg(&session_name);

    // Set starting directory if provided
    if let Some(dir) = cwd {
        let expanded = shellexpand::tilde(dir).to_string();
        cmd.arg("-c").arg(&expanded);
    }

    // Specify the shell to use (must be last argument)
    cmd.arg(&shell);

    let output = cmd.output()
        .map_err(|e| format!("Failed to execute tmux: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to create tmux session: {}", stderr));
    }

    info!("Created tmux session: {} (config: {:?})", session_name, config_path);
    Ok(session_name)
}

/// List all wiz-term tmux sessions
pub fn list_wizterm_sessions() -> Vec<TmuxSessionInfo> {
    let Some(mut cmd) = tmux_command() else {
        return Vec::new();
    };
    let output = cmd
        .arg("-L").arg(TMUX_SOCKET_NAME)
        .args(["list-sessions", "-F", "#{session_name}:#{session_created}:#{session_attached}"])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 3 {
                        let name = parts[0];
                        if name.starts_with(TMUX_SESSION_PREFIX) {
                            let session_id = name.strip_prefix(TMUX_SESSION_PREFIX)?.to_string();
                            let created_at = parts[1].parse().ok()?;
                            let attached = parts[2] != "0";
                            return Some(TmuxSessionInfo {
                                session_id,
                                tmux_session_name: name.to_string(),
                                created_at,
                                attached,
                            });
                        }
                    }
                    None
                })
                .collect()
        }
        Ok(output) => {
            // tmux might return error if no server is running (no sessions)
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("no server running") || stderr.contains("no sessions") {
                debug!("No tmux server running");
            } else {
                warn!("tmux list-sessions failed: {}", stderr);
            }
            Vec::new()
        }
        Err(e) => {
            warn!("Failed to execute tmux list-sessions: {}", e);
            Vec::new()
        }
    }
}

/// Check if a specific tmux session exists
pub fn session_exists(session_id: &str) -> bool {
    let session_name = format!("{}{}", TMUX_SESSION_PREFIX, session_id);
    tmux_command()
        .map(|mut cmd| {
            cmd.arg("-L").arg(TMUX_SOCKET_NAME)
                .args(["has-session", "-t", &session_name])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Kill a tmux session
pub fn kill_tmux_session(session_id: &str) -> Result<(), String> {
    let session_name = format!("{}{}", TMUX_SESSION_PREFIX, session_id);

    let mut cmd = tmux_command().ok_or("tmux not found")?;
    let output = cmd
        .arg("-L").arg(TMUX_SOCKET_NAME)
        .args(["kill-session", "-t", &session_name])
        .output()
        .map_err(|e| format!("Failed to execute tmux: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Don't error if session doesn't exist
        if !stderr.contains("session not found") && !stderr.contains("no server running") {
            return Err(format!("Failed to kill tmux session: {}", stderr));
        }
    }

    info!("Killed tmux session: {}", session_name);
    Ok(())
}

/// Get the command to attach to a tmux session
/// Uses dedicated socket and config file to ensure transparent settings are applied
pub fn get_attach_command(session_id: &str) -> Option<(String, Vec<String>)> {
    let tmux_path = get_tmux_path()?;
    let session_name = format!("{}{}", TMUX_SESSION_PREFIX, session_id);
    let config_path = get_config_path();

    Some((
        tmux_path.to_string_lossy().to_string(),
        vec![
            "-L".to_string(),
            TMUX_SOCKET_NAME.to_string(),
            "-f".to_string(),
            config_path.to_string_lossy().to_string(),
            "attach-session".to_string(),
            "-t".to_string(),
            session_name,
        ],
    ))
}

/// Info about a tmux session
#[derive(Debug, Clone)]
pub struct TmuxSessionInfo {
    pub session_id: String,
    pub tmux_session_name: String,
    pub created_at: i64,
    pub attached: bool,
}
