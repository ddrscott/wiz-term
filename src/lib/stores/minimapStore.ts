import { writable, get } from 'svelte/store';
import type { LayoutNode } from '$lib/types/terminal';
import { terminalCanvases } from '$lib/stores/terminalCanvases';

interface MinimapState {
	isOpen: boolean;
	isPinned: boolean;
}

interface PaneSnapshot {
	nodeId: string;
	sessionId?: string;
	imageData: string;
	type: 'terminal' | 'webview';
	url?: string;
	title?: string;
}

interface LayoutUpdate {
	layout: LayoutNode | null;
	snapshots: PaneSnapshot[];
	aspectRatio?: number; // width / height of main container
}

// Window handle (lazy loaded to avoid SSR issues)
let minimapWindow: any = null;
let unlistenReady: (() => void) | null = null;
let unlistenFocus: (() => void) | null = null;

// Capture callback - set by TerminalLanes (can be sync or async)
let captureCallback: (() => LayoutUpdate | Promise<LayoutUpdate>) | null = null;
let focusCallback: ((nodeId: string) => void) | null = null;

// Event-driven update scheduling (replaces polling)
let pendingRafId: number | null = null;
let updatesEnabled = false;

// Minimap dimensions for optimal thumbnail resolution
let minimapDimensions: { width: number; height: number } = { width: 300, height: 200 };

// Reusable temp canvases for capture (avoid creating new ones each frame)
const tempCanvasCache = new Map<string, { canvas: HTMLCanvasElement; ctx: CanvasRenderingContext2D }>();

function createMinimapStore() {
	const { subscribe, set, update } = writable<MinimapState>({
		isOpen: false,
		isPinned: true
	});

	async function openWindow(): Promise<boolean> {
		const state = get({ subscribe });
		if (state.isOpen && minimapWindow) {
			try {
				await minimapWindow.setFocus();
				return true;
			} catch {
				minimapWindow = null;
			}
		}

		try {
			const { WebviewWindow, getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
			const { emitTo, listen } = await import('@tauri-apps/api/event');

			// Check if window already exists (e.g., from HMR reload)
			try {
				const existing = WebviewWindow.getByLabel('minimap');
				if (existing) {
					await existing.close();
					await new Promise(resolve => setTimeout(resolve, 100));
				}
			} catch {
				// Window doesn't exist, that's fine
			}

			minimapWindow = new WebviewWindow('minimap', {
				url: '/minimap',
				title: 'Terminal Minimap',
				width: 300,
				height: 200,
				minWidth: 150,
				minHeight: 100,
				resizable: true,
				decorations: false,
				transparent: false,
				alwaysOnTop: true,
				focus: true
			});

			await new Promise<void>((resolve, reject) => {
				minimapWindow!.once('tauri://created', () => resolve());
				minimapWindow!.once('tauri://error', (e: any) => reject(e));
			});

			// Listen for window close
			minimapWindow.once('tauri://destroyed', () => {
				disableUpdates();
				update((s) => ({ ...s, isOpen: false }));
				minimapWindow = null;
			});

			// Set up listeners
			unlistenReady = await listen('minimap-ready', async () => {
				if (captureCallback) {
					try {
						const result = await captureCallback();
						emitTo('minimap', 'minimap-update', result);
					} catch (e) {
						console.warn('[Minimap] Failed to capture on ready:', e);
					}
				}
			});

			unlistenFocus = await listen<{ nodeId: string }>('minimap-focus', (event) => {
				focusCallback?.(event.payload.nodeId);
			});

			// Enable event-driven updates (terminals will call scheduleUpdate on output)
			enableUpdates();

			update((s) => ({ ...s, isOpen: true }));
			return true;
		} catch (e) {
			console.error('[Minimap] Failed to create window:', e);
			minimapWindow = null;
			return false;
		}
	}

	async function closeWindow(): Promise<void> {
		if (minimapWindow) {
			try {
				await minimapWindow.close();
			} catch {
				// Already closed
			}
			minimapWindow = null;
		}
		disableUpdates();
		update((s) => ({ ...s, isOpen: false }));
	}

	async function toggleWindow(): Promise<void> {
		const state = get({ subscribe });
		if (state.isOpen) {
			await closeWindow();
		} else {
			await openWindow();
		}
	}

	async function togglePinned(): Promise<void> {
		if (!minimapWindow) return;

		const state = get({ subscribe });
		const newPinned = !state.isPinned;

		try {
			await minimapWindow.setAlwaysOnTop(newPinned);
			update((s) => ({ ...s, isPinned: newPinned }));
		} catch (e) {
			console.error('[Minimap] Failed to toggle pin:', e);
		}
	}

	async function resetPosition(): Promise<void> {
		if (!minimapWindow) return;

		try {
			// Reset to default size and center on screen
			await minimapWindow.setSize({ type: 'Logical', width: 300, height: 200 });
			await minimapWindow.center();
			await minimapWindow.setFocus();
		} catch (e) {
			console.error('[Minimap] Failed to reset position:', e);
		}
	}

	// Perform the actual capture and send to minimap
	async function doUpdate(): Promise<void> {
		pendingRafId = null;

		if (!minimapWindow || !captureCallback || !updatesEnabled) {
			return;
		}

		try {
			// Consume all dirty flags before capturing
			terminalCanvases.consumeAllDirty();

			const { emitTo } = await import('@tauri-apps/api/event');

			// Query minimap window size directly (more reliable than events)
			try {
				const size = await minimapWindow.innerSize();
				if (size.width > 0 && size.height > 0) {
					minimapDimensions = { width: size.width, height: size.height };
				}
			} catch {
				// Window might be closing, ignore
			}

			// Capture using OffscreenAddon (works even when terminals are off-screen!)
			const result = await captureCallback();
			emitTo('minimap', 'minimap-update', result);
		} catch {
			// Ignore errors
		}
	}

	// Schedule a minimap update on next animation frame (debounced)
	// Called by terminals when they have new output
	function scheduleUpdate(): void {
		// Only schedule if minimap is open and no update is pending
		if (!updatesEnabled || !minimapWindow || pendingRafId !== null) {
			return;
		}

		pendingRafId = requestAnimationFrame(() => {
			doUpdate();
		});
	}

	function enableUpdates(): void {
		updatesEnabled = true;
	}

	function disableUpdates(): void {
		updatesEnabled = false;
		if (pendingRafId !== null) {
			cancelAnimationFrame(pendingRafId);
			pendingRafId = null;
		}
		unlistenReady?.();
		unlistenFocus?.();
		unlistenReady = null;
		unlistenFocus = null;
	}

	return {
		subscribe,
		openWindow,
		closeWindow,
		toggleWindow,
		togglePinned,
		resetPosition,
		scheduleUpdate,

		// Called by TerminalLanes to register capture callback
		setCaptureCallback(cb: () => LayoutUpdate | Promise<LayoutUpdate>) {
			captureCallback = cb;
		},

		// Called by TerminalLanes to register focus callback
		setFocusCallback(cb: (nodeId: string) => void) {
			focusCallback = cb;
		},

		// Helper to get temp canvas for capture
		getTempCanvas(nodeId: string, width: number, height: number) {
			let cached = tempCanvasCache.get(nodeId);
			if (!cached) {
				const canvas = document.createElement('canvas');
				const ctx = canvas.getContext('2d')!;
				cached = { canvas, ctx };
				tempCanvasCache.set(nodeId, cached);
			}
			if (cached.canvas.width !== width || cached.canvas.height !== height) {
				cached.canvas.width = width;
				cached.canvas.height = height;
			}
			return cached;
		},

		// Get minimap dimensions for optimal thumbnail resolution
		getMinimapDimensions(): { width: number; height: number } {
			return minimapDimensions;
		}
	};
}

export const minimapStore = createMinimapStore();
