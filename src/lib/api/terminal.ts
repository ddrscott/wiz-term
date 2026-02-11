import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
	TerminalSession,
	CreateSessionOptions,
	TerminalOutput,
	TerminalExit,
	TerminalPreferences,
	ReconnectableSession
} from '$lib/types/terminal';

export async function createSession(options?: CreateSessionOptions): Promise<TerminalSession> {
	return invoke('pty_create_session', { request: options || {} });
}

export async function writeToSession(sessionId: string, data: Uint8Array): Promise<void> {
	return invoke('pty_write', { sessionId, data: Array.from(data) });
}

export async function resizeSession(sessionId: string, cols: number, rows: number): Promise<void> {
	return invoke('pty_resize', { sessionId, cols, rows });
}

export async function killSession(sessionId: string): Promise<void> {
	return invoke('pty_kill', { sessionId });
}

export async function listSessions(): Promise<TerminalSession[]> {
	return invoke('pty_list_sessions');
}

export async function getSession(sessionId: string): Promise<TerminalSession | null> {
	return invoke('pty_get_session', { sessionId });
}

export async function onTerminalOutput(
	callback: (output: TerminalOutput) => void
): Promise<UnlistenFn> {
	return listen<TerminalOutput>('terminal-output', (event) => callback(event.payload));
}

export async function onTerminalExit(callback: (exit: TerminalExit) => void): Promise<UnlistenFn> {
	return listen<TerminalExit>('terminal-exit', (event) => callback(event.payload));
}

export async function saveLayout(layoutJson: string): Promise<void> {
	return invoke('pty_save_layout', { layoutJson });
}

export async function getLayout(): Promise<string | null> {
	return invoke('pty_get_layout');
}

export async function savePreferences(preferences: TerminalPreferences): Promise<void> {
	return invoke('pty_save_preferences', { preferences });
}

export async function getPreferences(): Promise<TerminalPreferences> {
	return invoke('pty_get_preferences');
}

/** Check if tmux is being used for session persistence */
export async function isUsingTmux(): Promise<boolean> {
	return invoke('pty_is_using_tmux');
}

/** List existing tmux sessions that can be reconnected to */
export async function listReconnectable(): Promise<ReconnectableSession[]> {
	return invoke('pty_list_reconnectable');
}

/** Reconnect to an existing tmux session */
export async function reconnectSession(
	sessionId: string,
	cols?: number,
	rows?: number
): Promise<TerminalSession> {
	return invoke('pty_reconnect_session', { sessionId, cols, rows });
}

/** Get the current tmux config content */
export async function getTmuxConfig(): Promise<string> {
	return invoke('pty_get_tmux_config');
}

/** Set the tmux config content */
export async function setTmuxConfig(content: string): Promise<void> {
	return invoke('pty_set_tmux_config', { content });
}

/** Reset tmux config to defaults and return the new content */
export async function resetTmuxConfig(): Promise<string> {
	return invoke('pty_reset_tmux_config');
}

/** Get the path to the tmux config file */
export async function getTmuxConfigPath(): Promise<string> {
	return invoke('pty_get_tmux_config_path');
}
