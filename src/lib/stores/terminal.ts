import { writable } from 'svelte/store';

// Simple event store to trigger new terminal creation
function createTerminalActions() {
	const { subscribe, set } = writable<number>(0);

	return {
		subscribe,
		requestNewTerminal() {
			// Increment to trigger reactive update
			set(Date.now());
		}
	};
}

export const terminalActions = createTerminalActions();
