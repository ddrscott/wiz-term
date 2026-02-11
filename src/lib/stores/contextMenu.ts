import { writable } from 'svelte/store';

export interface ContextMenuItem {
	label: string;
	icon?: string;
	action: () => void;
	disabled?: boolean;
}

export interface ContextMenuState {
	x: number;
	y: number;
	items: ContextMenuItem[];
}

function createContextMenuStore() {
	const { subscribe, set } = writable<ContextMenuState | null>(null);

	return {
		subscribe,
		show: (x: number, y: number, items: ContextMenuItem[]) => {
			set({ x, y, items });
		},
		close: () => {
			set(null);
		}
	};
}

export const contextMenuStore = createContextMenuStore();
