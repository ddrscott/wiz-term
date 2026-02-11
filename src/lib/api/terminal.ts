import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
	TerminalSession,
	CreateSessionOptions,
	TerminalOutput,
	TerminalExit,
	TerminalPreferences
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

/**
 * Save image data to a temp file and return the path.
 * Used for pasting/dropping images into the terminal for Claude Code.
 * @param data - Base64 encoded image data or data URL (data:image/png;base64,...)
 * @param filename - Optional original filename to preserve extension
 * @returns The path to the saved temp file
 */
export async function saveImageToTemp(data: string, filename?: string): Promise<string> {
	return invoke('save_temp_image', { data, filename });
}
