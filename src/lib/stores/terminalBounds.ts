import { writable } from 'svelte/store';

export interface TerminalBounds {
	x: number;
	y: number;
	width: number;
	height: number;
}

// Maps nodeId -> bounds (position relative to the lanes container)
const boundsMap = new Map<string, TerminalBounds>();

function createBoundsStore() {
	const { subscribe, set } = writable<Map<string, TerminalBounds>>(boundsMap);

	return {
		subscribe,

		// Called by LayoutSlot when its bounds change
		updateBounds(nodeId: string, bounds: TerminalBounds) {
			boundsMap.set(nodeId, bounds);
			set(new Map(boundsMap));
		},

		// Called when a terminal is removed
		removeBounds(nodeId: string) {
			boundsMap.delete(nodeId);
			set(new Map(boundsMap));
		},

		// Get bounds for a specific terminal (non-reactive, for initial positioning)
		getBounds(nodeId: string): TerminalBounds | undefined {
			return boundsMap.get(nodeId);
		},

		// Clear all bounds (e.g., on layout reset)
		clear() {
			boundsMap.clear();
			set(new Map(boundsMap));
		}
	};
}

export const terminalBounds = createBoundsStore();
