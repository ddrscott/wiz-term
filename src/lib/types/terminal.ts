export interface TerminalSession {
	id: string;
	command: string;
	args: string[];
	cwd: string | null;
	created_at: string;
	cols: number;
	rows: number;
	is_alive: boolean;
	/** Whether this session is backed by tmux (persistent across app restarts) */
	is_tmux: boolean;
}

/** A tmux session that can be reconnected to */
export interface ReconnectableSession {
	session_id: string;
	tmux_session_name: string;
	created_at: number;
	attached: boolean;
}

export interface CreateSessionOptions {
	command?: string;
	args?: string[];
	cwd?: string;
	cols?: number;
	rows?: number;
}

export interface TerminalOutput {
	session_id: string;
	data: number[];
}

export interface TerminalExit {
	session_id: string;
	exit_code: number | null;
}

// Layout tree types for split pane support

export type SplitDirection = 'horizontal' | 'vertical';
export type DropZone = 'left' | 'right' | 'top' | 'bottom' | 'center';

export interface TerminalNode {
	type: 'terminal';
	id: string;
	sessionId: string;
}

export interface SplitNode {
	type: 'split';
	id: string;
	direction: SplitDirection;
	children: LayoutNode[];
	sizes: number[]; // Percentages, e.g., [50, 50]
}

export type LayoutNode = TerminalNode | SplitNode;

export interface TerminalLayout {
	root: LayoutNode | null;
	version: number;
}

export interface DraggedTerminal {
	sessionId: string;
	sourceNodeId: string;
}

// Terminal preferences/settings
export interface TerminalPreferences {
	font_size: number;
	font_family: string;
	scrollback: number;
	cursor_blink: boolean;
	minimap_refresh_ms: number;
}

export const DEFAULT_TERMINAL_PREFERENCES: TerminalPreferences = {
	font_size: 13,
	font_family: 'JetBrains Mono',
	scrollback: 10000,
	cursor_blink: true,
	minimap_refresh_ms: 200
};

// Common monospace fonts for terminal
export const TERMINAL_FONTS = [
	'JetBrains Mono',
	'Fira Code',
	'Source Code Pro',
	'SF Mono',
	'Menlo',
	'Monaco',
	'Consolas',
	'Ubuntu Mono',
	'IBM Plex Mono',
	'Cascadia Code',
	'Hack',
	'Inconsolata'
] as const;
