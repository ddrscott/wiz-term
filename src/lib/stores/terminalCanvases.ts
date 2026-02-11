import { writable } from 'svelte/store';

interface CanvasEntry {
	container: HTMLElement;
	sessionId: string;
	nodeId: string;
	refreshCallback?: () => void;
}

// Direct map access for hot-path operations (no subscribe/unsubscribe overhead)
const canvasMap = new Map<string, CanvasEntry>();

// Track which terminals have new output (dirty flag for minimap optimization)
const dirtySet = new Set<string>();

// Refresh callbacks for forcing canvas updates
const refreshCallbacks = new Map<string, () => void>();

// Serialize callbacks for getting terminal buffer (works off-screen)
const serializeCallbacks = new Map<string, () => string>();

// Capture callbacks for getting terminal image via OffscreenAddon (works off-screen!)
// Callbacks can be sync or async - sync is preferred for performance
// Optional target dimensions allow capturing at optimal resolution for display
const captureCallbacks = new Map<string, (targetWidth?: number, targetHeight?: number) => string | Promise<string>>();

// Terminal dimensions for rendering
const terminalDimensions = new Map<string, { cols: number; rows: number }>();

function createCanvasRegistry() {
	const { subscribe, set } = writable<Map<string, CanvasEntry>>(canvasMap);

	return {
		subscribe,
		register(nodeId: string, sessionId: string, container: HTMLElement) {
			canvasMap.set(nodeId, { container, sessionId, nodeId });
			dirtySet.add(nodeId); // Mark as dirty on registration
			set(new Map(canvasMap)); // Trigger reactivity for subscribers
		},
		unregister(nodeId: string) {
			canvasMap.delete(nodeId);
			dirtySet.delete(nodeId);
			set(new Map(canvasMap)); // Trigger reactivity for subscribers
		},
		// Get the container element for a terminal (direct map access)
		getContainer(nodeId: string): HTMLElement | null {
			return canvasMap.get(nodeId)?.container ?? null;
		},

		// Get the main canvas from a terminal container (direct map access)
		getCanvas(nodeId: string): HTMLCanvasElement | null {
			const entry = canvasMap.get(nodeId);
			if (!entry?.container) return null;

			// Find the largest canvas (main rendering canvas, skip xterm-link-layer)
			const canvases = entry.container.querySelectorAll('canvas:not(.xterm-link-layer)');
			let mainCanvas: HTMLCanvasElement | null = null;
			let maxArea = 0;
			canvases.forEach((c) => {
				const canvas = c as HTMLCanvasElement;
				const area = canvas.width * canvas.height;
				if (area > maxArea) {
					maxArea = area;
					mainCanvas = canvas;
				}
			});
			return mainCanvas;
		},

		// Mark a terminal as having new output (called by TerminalLane on output)
		markDirty(nodeId: string) {
			dirtySet.add(nodeId);
		},

		// Check if terminal is dirty and clear the flag (called by minimap before capture)
		consumeDirty(nodeId: string): boolean {
			if (dirtySet.has(nodeId)) {
				dirtySet.delete(nodeId);
				return true;
			}
			return false;
		},

		// Check if any terminal is dirty (for detached minimap)
		hasAnyDirty(): boolean {
			return dirtySet.size > 0;
		},

		// Get all dirty nodeIds and clear them
		consumeAllDirty(): string[] {
			const dirty = Array.from(dirtySet);
			dirtySet.clear();
			return dirty;
		},

		// Register a refresh callback for a terminal (called by TerminalLane)
		registerRefreshCallback(nodeId: string, callback: () => void) {
			refreshCallbacks.set(nodeId, callback);
		},

		// Unregister a refresh callback
		unregisterRefreshCallback(nodeId: string) {
			refreshCallbacks.delete(nodeId);
		},

		// Force refresh all terminals (call before minimap capture)
		refreshAll() {
			for (const callback of refreshCallbacks.values()) {
				callback();
			}
		},

		// Get all registered node IDs
		getAllNodeIds(): string[] {
			return Array.from(canvasMap.keys());
		},

		// Register a serialize callback for a terminal
		registerSerializeCallback(nodeId: string, callback: () => string, cols: number, rows: number) {
			serializeCallbacks.set(nodeId, callback);
			terminalDimensions.set(nodeId, { cols, rows });
		},

		// Update terminal dimensions
		updateDimensions(nodeId: string, cols: number, rows: number) {
			terminalDimensions.set(nodeId, { cols, rows });
		},

		// Unregister a serialize callback
		unregisterSerializeCallback(nodeId: string) {
			serializeCallbacks.delete(nodeId);
			terminalDimensions.delete(nodeId);
		},

		// Get serialized terminal content (works even when off-screen!)
		getSerializedContent(nodeId: string): string | null {
			const callback = serializeCallbacks.get(nodeId);
			return callback ? callback() : null;
		},

		// Get terminal dimensions
		getDimensions(nodeId: string): { cols: number; rows: number } | null {
			return terminalDimensions.get(nodeId) ?? null;
		},

		// Register a capture callback for a terminal (via OffscreenAddon)
		// Sync callbacks are preferred for performance, but async is also supported
		// Callback receives optional target dimensions for optimal resolution
		registerCaptureCallback(nodeId: string, callback: (targetWidth?: number, targetHeight?: number) => string | Promise<string>) {
			captureCallbacks.set(nodeId, callback);
		},

		// Unregister a capture callback
		unregisterCaptureCallback(nodeId: string) {
			captureCallbacks.delete(nodeId);
		},

		// Capture terminal image via OffscreenAddon (works even when off-screen!)
		// Optional target dimensions allow capturing at optimal resolution
		async captureImage(nodeId: string, targetWidth?: number, targetHeight?: number): Promise<string | null> {
			const callback = captureCallbacks.get(nodeId);
			if (callback) {
				try {
					return await callback(targetWidth, targetHeight);
				} catch (e) {
					console.warn('Failed to capture terminal image:', e);
					return null;
				}
			}
			return null;
		},

		// Check if capture callback is registered for a terminal
		hasCaptureCallback(nodeId: string): boolean {
			return captureCallbacks.has(nodeId);
		}
	};
}

export const terminalCanvases = createCanvasRegistry();
