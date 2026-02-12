<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { get } from 'svelte/store';
	import {
		createSession,
		listSessions,
		onTerminalExit,
		saveLayout,
		getLayout
	} from '$lib/api/terminal';
	import { terminalActions } from '$lib/stores/terminal';
	import { settings } from '$lib/stores/settings';
	import { terminalCanvases } from '$lib/stores/terminalCanvases';
	import { minimapStore } from '$lib/stores/minimapStore';
	import { terminalBounds } from '$lib/stores/terminalBounds';
	import SplitContainer from './SplitContainer.svelte';
	import TerminalLane from './TerminalLane.svelte';
	import WebviewPane from '$lib/components/webview/WebviewPane.svelte';
	import type {
		TerminalSession,
		TerminalLayout,
		DropZone,
		LayoutNode,
		WebviewNode
	} from '$lib/types/terminal';
	import {
		createEmptyLayout,
		createLayoutWithTerminal,
		addTerminal,
		addWebview,
		splitNode,
		insertTerminalAfter,
		removeSession,
		removeNode,
		resizeSplit,
		getAllSessionIds,
		getAllWebviews,
		updateWebview,
		serializeLayout,
		deserializeLayout,
		getFirstTerminal,
		findNodeBySessionId,
		findRootColumnId,
		findNodeById
	} from '$lib/utils/terminalLayout';

	interface Props {
		visible?: boolean;
	}

	let { visible = true }: Props = $props();

	// Layout tree and sessions map
	let layout = $state<TerminalLayout>(createEmptyLayout());
	let sessions = $state<Map<string, TerminalSession>>(new Map());
	let loading = $state(true);
	let unlistenExit: (() => void) | null = null;

	// Focus tracking for keyboard shortcuts
	let focusedNodeId = $state<string | null>(null);

	// Reference to lanes container for aspect ratio calculation
	let lanesContainerEl = $state<HTMLDivElement | null>(null);

	// Column widths for horizontal scrolling (stored by node id)
	let columnWidths = $state<Map<string, number>>(new Map());
	const DEFAULT_COLUMN_WIDTH = 640; // 80 columns at default font

	// Counter to force bounds recalculation when column widths change
	let boundsGeneration = $state(0);

	// Debounced layout save
	let saveTimeout: ReturnType<typeof setTimeout> | null = null;

	// Immediate layout save - use for critical operations (session create/delete)
	async function saveLayoutNow() {
		if (saveTimeout) clearTimeout(saveTimeout);
		try {
			await saveLayout(serializeLayout(layout));
		} catch (e) {
			console.error('Failed to save layout:', e);
		}
	}

	// Debounced layout save - use for frequent updates (resize, reorder)
	function debouncedSaveLayout() {
		if (saveTimeout) clearTimeout(saveTimeout);
		saveTimeout = setTimeout(async () => {
			try {
				await saveLayout(serializeLayout(layout));
			} catch (e) {
				console.error('Failed to save layout:', e);
			}
		}, 500);
	}

	// Helper to get all node IDs from layout tree (terminals and webviews)
	function getAllNodeIds(node: LayoutNode | null): string[] {
		if (!node) return [];
		if (node.type === 'terminal' || node.type === 'webview') return [node.id];
		if (node.type === 'split') {
			return node.children.flatMap(getAllNodeIds);
		}
		return [];
	}

	// Capture snapshots for minimap from terminal canvases and webviews
	// Uses OffscreenAddon for reliable terminal capture even when off-screen
	async function captureSnapshots() {
		const nodeIds = getAllNodeIds(layout.root);
		const webviews = getAllWebviews(layout);
		const snapshots: { nodeId: string; sessionId?: string; imageData: string; type: 'terminal' | 'webview'; url?: string; title?: string }[] = [];

		// Get minimap dimensions - capture at exactly the display size
		const minimapDims = minimapStore.getMinimapDimensions();
		const targetWidth = minimapDims.width;
		const targetHeight = minimapDims.height;

		// Capture all terminals in parallel using OffscreenAddon
		const terminalPromises = nodeIds
			.filter(nodeId => !webviews.some(w => w.id === nodeId))
			.map(async (nodeId) => {
				// Try OffscreenAddon capture first (works off-screen!)
				if (terminalCanvases.hasCaptureCallback(nodeId)) {
					try {
						const imageData = await terminalCanvases.captureImage(nodeId, targetWidth, targetHeight);
						// Validate image data: must be a data URL and have reasonable content
						if (imageData && imageData.startsWith('data:image/') && imageData.length > 500) {
							return { nodeId, sessionId: nodeId, imageData, type: 'terminal' as const };
						}
					} catch {
						// Fall through to canvas capture
					}
				}

				// Fallback to canvas capture (only works when visible)
				const sourceCanvas = terminalCanvases.getCanvas(nodeId);
				if (sourceCanvas && sourceCanvas.width > 0 && sourceCanvas.height > 0) {
					try {
						const { canvas: tempCanvas, ctx } = minimapStore.getTempCanvas(
							nodeId,
							sourceCanvas.width,
							sourceCanvas.height
						);
						ctx.fillStyle = '#0a0a0f';
						ctx.fillRect(0, 0, tempCanvas.width, tempCanvas.height);
						ctx.drawImage(sourceCanvas, 0, 0);
						const imageData = tempCanvas.toDataURL(); // PNG - faster encoding
						if (imageData.length > 500) {
							return { nodeId, sessionId: nodeId, imageData, type: 'terminal' as const };
						}
					} catch {
						// Ignore capture errors
					}
				}
				return null;
			});

		// For webviews, just send metadata (placeholder shown in minimap)
		// Actual screenshot capture would be slow and has cross-origin issues
		const webviewPromises = webviews.map(async (webview) => {
			return {
				nodeId: webview.id,
				imageData: '', // No screenshot - minimap shows styled placeholder
				type: 'webview' as const,
				url: webview.url,
				title: webview.title
			};
		});

		const [terminalResults, webviewResults] = await Promise.all([
			Promise.all(terminalPromises),
			Promise.all(webviewPromises)
		]);

		for (const result of terminalResults) {
			if (result) {
				snapshots.push(result);
			}
		}
		for (const result of webviewResults) {
			if (result) {
				snapshots.push(result);
			}
		}

		// Calculate aspect ratio of the full content (including scrolled areas)
		let aspectRatio: number | undefined;
		if (lanesContainerEl) {
			// Use scrollWidth/scrollHeight to get full content size, not just visible bounds
			const width = lanesContainerEl.scrollWidth;
			const height = lanesContainerEl.scrollHeight;
			if (width > 0 && height > 0) {
				aspectRatio = width / height;
			}
		}

		return { layout: layout.root, snapshots, aspectRatio };
	}


	onMount(async () => {
		console.log('[TerminalLanes] onMount started');

		// Load settings first to get shell_path
		await settings.load();

		// Register minimap callbacks
		minimapStore.setCaptureCallback(captureSnapshots);
		minimapStore.setFocusCallback(handleFocus);

		// Load any already-active PTY sessions from backend
		// This handles frontend refresh while backend still has active sessions
		let existingSessions: TerminalSession[] = [];
		try {
			console.log('[TerminalLanes] Calling listSessions...');
			existingSessions = await listSessions();
			console.log(`[TerminalLanes] Found ${existingSessions.length} existing PTY sessions`);
		} catch (e) {
			console.error('[TerminalLanes] Failed to load terminal sessions:', e);
		}

		// Create sessions map
		const sessionMap = new Map<string, TerminalSession>();
		for (const s of existingSessions) {
			sessionMap.set(s.id, s);
		}
		sessions = sessionMap;

		// Try to load saved layout
		try {
			const savedLayoutJson = await getLayout();
			if (savedLayoutJson) {
				const savedLayout = deserializeLayout(savedLayoutJson);
				if (savedLayout) {
					// Validate that all sessions in layout still exist
					const layoutSessionIds = getAllSessionIds(savedLayout);
					const validSessionIds = layoutSessionIds.filter((id) => sessions.has(id));

					if (validSessionIds.length > 0) {
						// Use saved layout but remove any stale session references
						layout = savedLayout;
						// Clean up stale sessions from layout
						for (const id of layoutSessionIds) {
							if (!sessions.has(id)) {
								layout = removeSession(layout, id);
							}
						}
					}
				}
			}
		} catch (e) {
			console.error('Failed to load layout:', e);
		}

		// Add any existing sessions that aren't in the layout
		const layoutSessionIdSet = new Set(getAllSessionIds(layout));
		let addedSessions = false;
		for (const [sessionId] of sessions) {
			if (!layoutSessionIdSet.has(sessionId)) {
				console.log(`Adding session to layout: ${sessionId}`);
				if (!layout.root) {
					layout = createLayoutWithTerminal(sessionId);
				} else {
					layout = addTerminal(layout, sessionId);
				}
				addedSessions = true;
			}
		}

		// Save layout if we added sessions
		if (addedSessions) {
			await saveLayoutNow();
		}

		// If no layout (no sessions at all), create a new session
		if (!layout.root) {
			console.log('[TerminalLanes] No layout.root, creating new session...');
			await handleNewSession();
			console.log('[TerminalLanes] handleNewSession completed');
		}

		// Set initial focus
		const firstTerminal = getFirstTerminal(layout);
		if (firstTerminal) {
			focusedNodeId = firstTerminal.id;
		}

		// Listen for session exits to update status
		unlistenExit = await onTerminalExit((exit) => {
			const session = sessions.get(exit.session_id);
			if (session) {
				sessions.set(exit.session_id, { ...session, is_alive: false });
				sessions = new Map(sessions);
			}
		});

		loading = false;
	});

	onDestroy(() => {
		unlistenExit?.();
		if (saveTimeout) clearTimeout(saveTimeout);
	});

	// Listen for new terminal requests from header
	$effect(() => {
		const unsub = terminalActions.subscribe((timestamp) => {
			if (timestamp > 0 && !loading) {
				handleNewSession();
			}
		});
		return unsub;
	});

	async function handleNewSession(targetNodeId?: string, zone?: DropZone) {
		console.log('[TerminalLanes] handleNewSession called', { targetNodeId, zone });
		try {
			// Get shell path from settings
			const currentSettings = get(settings);
			const shellPath = currentSettings.terminal.shell_path || '/bin/zsh';
			console.log('[TerminalLanes] Calling createSession with shell:', shellPath);
			const session = await createSession({
				command: shellPath,
				args: ['-l'] // Login shell for proper environment
			});
			console.log('[TerminalLanes] Session created:', session.id);
			sessions.set(session.id, session);
			sessions = new Map(sessions);

			if (!layout.root) {
				// First session - create layout
				layout = createLayoutWithTerminal(session.id);
			} else if (targetNodeId && zone) {
				// Horizontal splits (left/right) create independent columns
				// Vertical splits (top/bottom) create nested splits within columns
				if (zone === 'left' || zone === 'right') {
					// Create new independent column - maintains flat horizontal layout
					layout = insertTerminalAfter(layout, targetNodeId, session.id);
				} else {
					// Vertical split - nest within the column
					layout = splitNode(layout, targetNodeId, zone, session.id);
				}
			} else {
				// Add to root level (Cmd+N) - maintains flat horizontal layout
				layout = addTerminal(layout, session.id);
			}

			// Focus the new terminal
			const newNode = findNodeBySessionId(layout, session.id);
			if (newNode) {
				focusedNodeId = newNode.id;
			}

			// Save immediately - don't risk losing session to HMR/crash
			await saveLayoutNow();
		} catch (e) {
			console.error('[TerminalLanes] Failed to create terminal session:', e);
			// Dispatch custom event so layout can show settings panel
			window.dispatchEvent(new CustomEvent('terminal-creation-failed', {
				detail: { error: e instanceof Error ? e.message : String(e) }
			}));
		}
	}

	async function handleCloseSession(sessionId: string) {
		layout = removeSession(layout, sessionId);
		sessions.delete(sessionId);
		sessions = new Map(sessions);

		// Update focus if needed
		if (focusedNodeId) {
			const node = findNodeBySessionId(layout, sessionId);
			if (node && node.id === focusedNodeId) {
				const first = getFirstTerminal(layout);
				focusedNodeId = first?.id ?? null;
			}
		}

		// If no terminals left, create a new one
		if (!layout.root) {
			await handleNewSession();
			return; // handleNewSession already saves
		}

		// Save immediately - don't risk stale session reappearing on reload
		await saveLayoutNow();
	}

	function handleResize(nodeId: string, sizes: number[]) {
		layout = resizeSplit(layout, nodeId, sizes);
		debouncedSaveLayout();
	}

	function handleColumnWidthChange(nodeId: string, width: number) {
		columnWidths.set(nodeId, width);
		columnWidths = new Map(columnWidths);
		// Force bounds recalculation after column width changes
		boundsGeneration++;
	}

	function handleFocus(nodeId: string) {
		focusedNodeId = nodeId;
		// Smooth scroll the terminal into view
		const container = terminalCanvases.getContainer(nodeId);
		if (container) {
			container.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'nearest' });
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		// Cmd+D - split horizontally (new pane on right)
		if ((e.metaKey || e.ctrlKey) && e.key === 'd' && !e.shiftKey) {
			e.preventDefault();
			if (focusedNodeId) {
				handleNewSession(focusedNodeId, 'right');
			}
		}

		// Cmd+Shift+D - split vertically (new pane below)
		if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'd') {
			e.preventDefault();
			if (focusedNodeId) {
				handleNewSession(focusedNodeId, 'bottom');
			}
		}

		// Cmd+N - new terminal
		if ((e.metaKey || e.ctrlKey) && e.key === 'n') {
			e.preventDefault();
			handleNewSession();
		}

		// Cmd+W - close focused pane (terminal or webview)
		if ((e.metaKey || e.ctrlKey) && e.key === 'w') {
			e.preventDefault();
			if (focusedNodeId) {
				// Check if it's a terminal
				const sessionId = getAllSessionIds(layout).find((sid) => {
					const n = findNodeBySessionId(layout, sid);
					return n?.id === focusedNodeId;
				});
				if (sessionId) {
					handleCloseSession(sessionId);
					return;
				}

				// Check if it's a webview
				const webview = getAllWebviews(layout).find((w) => w.id === focusedNodeId);
				if (webview) {
					handleCloseWebview(webview.id);
				}
			}
		}
	}

	// Open a URL in a webview pane to the right of the focused terminal
	function handleOpenWebview(url: string, title?: string) {
		layout = addWebview(layout, url, title, focusedNodeId ?? undefined);

		// Find the new webview and focus it
		const webviews = getAllWebviews(layout);
		const newWebview = webviews.find((w) => w.url === url);
		if (newWebview) {
			focusedNodeId = newWebview.id;
		}

		debouncedSaveLayout();
	}

	async function handleCloseWebview(nodeId: string) {
		// Close the native webview first (don't wait for onDestroy)
		try {
			await invoke('close_webview', { id: `browser-${nodeId}` });
		} catch (e) {
			// Webview might already be closed
			console.warn('Failed to close webview:', e);
		}

		// Remove stale bounds entry
		terminalBounds.removeBounds(nodeId);

		// Then remove from layout
		layout = removeNode(layout, nodeId);

		// Force bounds recalculation
		boundsGeneration++;

		// Update focus if needed
		if (focusedNodeId === nodeId) {
			const first = getFirstTerminal(layout);
			focusedNodeId = first?.id ?? null;
		}

		debouncedSaveLayout();
	}

	function handleWebviewUrlChange(nodeId: string, url: string) {
		layout = updateWebview(layout, nodeId, { url });
		debouncedSaveLayout();
	}

	function handleWebviewTitleChange(nodeId: string, title: string) {
		layout = updateWebview(layout, nodeId, { title });
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="terminal-lanes-wrapper">
	<div class="lanes-container" bind:this={lanesContainerEl}>
		{#if loading}
			<div class="loading-initial">
				<div class="spinner"></div>
				<span>Loading terminals...</span>
			</div>
		{:else if !layout.root}
			<div class="empty-state">
				<span>No terminal sessions</span>
				<p>Click "new terminal" to create one</p>
			</div>
		{:else}
			<!-- Layout slots - just measure positions, no terminals inside -->
			<SplitContainer
				node={layout.root}
				{lanesContainerEl}
				{focusedNodeId}
				onResize={handleResize}
				onFocus={handleFocus}
				isRoot={true}
				{columnWidths}
				onColumnWidthChange={handleColumnWidthChange}
				{boundsGeneration}
			/>

			<!-- Terminal registry - all terminals rendered flat, positioned absolutely -->
			<!-- IMPORTANT: Always render terminals to prevent xterm.js destruction on layout changes -->
			<!-- Terminals without bounds are hidden but kept alive -->
			<div class="terminal-registry">
				{#each [...sessions.entries()] as [sessionId, session] (sessionId)}
					{@const node = findNodeBySessionId(layout, sessionId)}
					{@const bounds = node ? $terminalBounds.get(node.id) : undefined}
					{@const hasBounds = bounds && bounds.width > 0 && bounds.height > 0}
					{#if node}
						<div
							class="terminal-wrapper"
							class:hidden={!hasBounds}
							style:left="{hasBounds ? bounds.x : 0}px"
							style:top="{hasBounds ? bounds.y : 0}px"
							style:width="{hasBounds ? bounds.width : 1}px"
							style:height="{hasBounds ? bounds.height : 1}px"
						>
							<TerminalLane
								{session}
								nodeId={node.id}
								{visible}
								onClose={() => handleCloseSession(sessionId)}
								onWidthChange={(width) => {
									const columnId = findRootColumnId(layout, node.id);
									if (columnId) handleColumnWidthChange(columnId, width);
								}}
								onFocus={handleFocus}
								onOpenWebview={handleOpenWebview}
							/>
						</div>
					{/if}
				{/each}

				<!-- Webview registry - webviews positioned absolutely like terminals -->
				{#each getAllWebviews(layout) as webview (webview.id)}
					{@const bounds = $terminalBounds.get(webview.id)}
					{@const hasBounds = bounds && bounds.width > 0 && bounds.height > 0}
					<div
						class="terminal-wrapper"
						class:hidden={!hasBounds}
						style:left="{hasBounds ? bounds.x : 0}px"
						style:top="{hasBounds ? bounds.y : 0}px"
						style:width="{hasBounds ? bounds.width : 1}px"
						style:height="{hasBounds ? bounds.height : 1}px"
					>
						<WebviewPane
							nodeId={webview.id}
							url={webview.url}
							title={webview.title}
							bounds={hasBounds ? bounds : undefined}
							onClose={() => handleCloseWebview(webview.id)}
							onUrlChange={(url) => handleWebviewUrlChange(webview.id, url)}
							onTitleChange={(title) => handleWebviewTitleChange(webview.id, title)}
							onFocus={handleFocus}
							onWidthChange={(width) => {
								const columnId = findRootColumnId(layout, webview.id);
								if (columnId) handleColumnWidthChange(columnId, width);
							}}
						/>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>

<style>
	.terminal-lanes-wrapper {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		user-select: none;
		overflow: hidden;
		overscroll-behavior: none;
	}

	.lanes-container {
		position: relative;
		flex: 1;
		display: flex;
		min-height: 0;
		overflow-x: auto;
		overflow-y: hidden;
		background: var(--terminal-bg);
		user-select: none;
		/* Contain scroll - don't propagate to parent or trigger bounce */
		overscroll-behavior: contain;
	}

	.terminal-registry {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		pointer-events: none;
	}

	.terminal-wrapper {
		position: absolute;
		pointer-events: auto;
		display: flex;
		flex-direction: column;
	}

	.terminal-wrapper.hidden {
		visibility: hidden;
		pointer-events: none;
	}

	.loading-initial {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
		gap: 12px;
		color: #64748b;
		font-size: 13px;
	}

	.spinner {
		width: 24px;
		height: 24px;
		border: 2px solid #1e1e2e;
		border-top-color: #da7756;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
		gap: 8px;
		color: #64748b;
	}

	.empty-state span {
		font-size: 14px;
		color: #e2e8f0;
	}

	.empty-state p {
		font-size: 12px;
		margin: 0;
	}
</style>
