import { writable, get } from 'svelte/store';
import type { TerminalPreferences, DEFAULT_TERMINAL_PREFERENCES } from '$lib/types/terminal';
import { getPreferences, savePreferences } from '$lib/api/terminal';

interface Settings {
	terminal: TerminalPreferences;
}

const defaultSettings: Settings = {
	terminal: {
		font_size: 13,
		font_family: 'SF Mono',
		scrollback: 10000,
		cursor_blink: true,
		minimap_refresh_ms: 200,
		use_webgl: true,
		shell_path: '/bin/zsh'
	}
};

function createSettingsStore() {
	const { subscribe, set, update } = writable<Settings>(defaultSettings);

	return {
		subscribe,

		async load() {
			try {
				const prefs = await getPreferences();
				update((s) => ({ ...s, terminal: prefs }));
			} catch (e) {
				console.error('Failed to load preferences:', e);
			}
		},

		async updateTerminal(prefs: Partial<TerminalPreferences>) {
			const current = get({ subscribe });
			const newPrefs = { ...current.terminal, ...prefs };
			update((s) => ({ ...s, terminal: newPrefs }));
			try {
				await savePreferences(newPrefs);
			} catch (e) {
				console.error('Failed to save preferences:', e);
			}
		}
	};
}

export const settings = createSettingsStore();
