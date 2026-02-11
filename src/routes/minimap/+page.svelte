<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import { getCurrentWindow } from '@tauri-apps/api/window';

	interface PaneSnapshot {
		nodeId: string;
		sessionId?: string;
		imageData: string;
		type: 'terminal' | 'webview';
		url?: string; // For webviews
		title?: string; // For webviews
	}

	interface LayoutUpdate {
		layout: LayoutNode | null;
		snapshots: PaneSnapshot[];
		aspectRatio?: number;
	}

	interface LayoutNode {
		type: 'terminal' | 'webview' | 'split';
		id: string;
		sessionId?: string;
		url?: string; // For webviews
		title?: string; // For webviews
		direction?: 'horizontal' | 'vertical';
		children?: LayoutNode[];
		sizes?: number[];
	}

	let layout = $state<LayoutNode | null>(null);
	let snapshots = $state<Map<string, PaneSnapshot>>(new Map());
	let aspectRatio = $state<number | undefined>(undefined);
	let alwaysOnTop = $state(true);
	let unlistenUpdate: (() => void) | null = null;

	// Track drag state to prevent click on drag
	let dragStartPos = $state<{ x: number; y: number } | null>(null);
	let didDrag = $state(false);
	const DRAG_THRESHOLD = 5; // pixels of movement to consider it a drag
	let currentAspectRatio: number | undefined = undefined;
	let isResizing = false;
	let resizeHandler: (() => void) | null = null;

	// Enforce aspect ratio on window resize
	async function enforceAspectRatio(newAspectRatio: number) {
		// Validate aspect ratio to prevent NaN/Infinity issues
		if (isResizing || !newAspectRatio || !Number.isFinite(newAspectRatio) || newAspectRatio <= 0) {
			return;
		}
		isResizing = true;

		try {
			const win = getCurrentWindow();
			const size = await win.innerSize();

			// Validate size values
			if (!size.width || !size.height || size.width <= 0) {
				return;
			}

			// Keep width, adjust height to match aspect ratio
			const newHeight = Math.round(size.width / newAspectRatio);

			// Validate calculated height and enforce reasonable bounds
			if (!Number.isFinite(newHeight) || newHeight < 50 || newHeight > 10000) {
				return;
			}

			if (Math.abs(size.height - newHeight) > 2) {
				await win.setSize({ type: 'Logical', width: size.width, height: newHeight });
			}
		} catch {
			// Ignore errors
		} finally {
			isResizing = false;
		}
	}

	onMount(async () => {
		const win = getCurrentWindow();

		// Listen for layout updates from main window
		unlistenUpdate = await listen<LayoutUpdate>('minimap-update', async (event) => {
			layout = event.payload.layout;
			const newSnapshots = new Map<string, PaneSnapshot>();
			for (const snap of event.payload.snapshots) {
				newSnapshots.set(snap.nodeId, snap);
			}
			snapshots = newSnapshots;

			// Update aspect ratio and enforce on window (with validation)
			const newRatio = event.payload.aspectRatio;
			if (newRatio && Number.isFinite(newRatio) && newRatio > 0 && newRatio !== currentAspectRatio) {
				currentAspectRatio = newRatio;
				aspectRatio = newRatio;
				await enforceAspectRatio(newRatio);
			}
		});

		// Enforce aspect ratio when window is resized by user
		resizeHandler = () => {
			if (currentAspectRatio && Number.isFinite(currentAspectRatio)) {
				enforceAspectRatio(currentAspectRatio);
			}
		};
		window.addEventListener('resize', resizeHandler);

		// Request initial data from main window
		await win.emit('minimap-ready');
	});

	onDestroy(() => {
		unlistenUpdate?.();
		// Clean up resize listener to prevent memory leaks
		if (resizeHandler) {
			window.removeEventListener('resize', resizeHandler);
			resizeHandler = null;
		}
	});

	async function toggleAlwaysOnTop() {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		const win = getCurrentWindow();
		alwaysOnTop = !alwaysOnTop;
		await win.setAlwaysOnTop(alwaysOnTop);
	}

	async function closeWindow() {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		const win = getCurrentWindow();
		await win.close();
	}

	async function startDrag(e?: MouseEvent) {
		// Track initial position to detect drags
		if (e) {
			dragStartPos = { x: e.screenX, y: e.screenY };
			didDrag = false;
		}
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		const win = getCurrentWindow();
		await win.startDragging();
	}

	function handleMouseUp(e: MouseEvent) {
		if (dragStartPos) {
			const dx = Math.abs(e.screenX - dragStartPos.x);
			const dy = Math.abs(e.screenY - dragStartPos.y);
			didDrag = dx > DRAG_THRESHOLD || dy > DRAG_THRESHOLD;
		}
		dragStartPos = null;
	}

	async function handleNodeClick(nodeId: string) {
		// Don't fire click if user was dragging the window
		if (didDrag) {
			didDrag = false;
			return;
		}
		const win = getCurrentWindow();
		await win.emit('minimap-focus', { nodeId });
	}
</script>

<div class="minimap-window" onmousedown={(e) => startDrag(e)} onmouseup={handleMouseUp}>
	<!-- Drag border frame -->
	<div class="drag-frame drag-top" onmousedown={(e) => startDrag(e)}></div>
	<div class="drag-frame drag-bottom" onmousedown={(e) => startDrag(e)}></div>
	<div class="drag-frame drag-left" onmousedown={(e) => startDrag(e)}></div>
	<div class="drag-frame drag-right" onmousedown={(e) => startDrag(e)}></div>

	<!-- Hover controls overlay -->
	<div class="hover-controls" onmousedown={(e) => e.stopPropagation()}>
		<button
			class="control-btn"
			class:active={alwaysOnTop}
			onclick={toggleAlwaysOnTop}
			title={alwaysOnTop ? 'Unpin from top' : 'Pin to top'}
		>
			{alwaysOnTop ? '📌' : '📍'}
		</button>
		<button class="control-btn close" onclick={closeWindow} title="Close">×</button>
	</div>

	<div class="content" onmousedown={(e) => startDrag(e)}>
		<div
			class="aspect-container"
			style={aspectRatio ? `--aspect-ratio: ${aspectRatio}; aspect-ratio: ${aspectRatio};` : ''}
		>
			{#if layout}
				{@render renderNode(layout)}
			{:else}
				<div class="empty">Waiting for terminals...</div>
			{/if}
		</div>
	</div>
</div>

{#snippet renderNode(node: LayoutNode)}
	{#if node.type === 'terminal'}
		{@const snap = snapshots.get(node.id)}
		<button class="pane-thumb terminal-thumb" onclick={() => handleNodeClick(node.id)} onmousedown={(e) => startDrag(e)}>
			{#if snap?.imageData}
				<img src={snap.imageData} alt="Terminal" draggable="false" />
			{:else}
				<div class="placeholder"></div>
			{/if}
		</button>
	{:else if node.type === 'webview'}
		{@const snap = snapshots.get(node.id)}
		<button class="pane-thumb webview-thumb" onclick={() => handleNodeClick(node.id)} onmousedown={(e) => startDrag(e)}>
			{#if snap?.imageData}
				<img src={snap.imageData} alt="Webview" draggable="false" />
			{:else}
				<div class="webview-placeholder">
					<span class="webview-icon">◧</span>
					<span class="webview-url">{node.title || node.url || 'Browser'}</span>
				</div>
			{/if}
		</button>
	{:else if node.children}
		<div
			class="split-container"
			class:horizontal={node.direction === 'horizontal'}
			class:vertical={node.direction === 'vertical'}
			onmousedown={(e) => startDrag(e)}
		>
			{#each node.children as child, i (child.id)}
				<div class="split-child" style="flex: {node.sizes?.[i] ?? 1};" onmousedown={(e) => startDrag(e)}>
					{@render renderNode(child)}
				</div>
			{/each}
		</div>
	{/if}
{/snippet}

<svelte:head>
	<style>
		html, body {
			margin: 0;
			padding: 0;
			overflow: hidden;
			background: #0f0f1a;
		}
	</style>
</svelte:head>

<style>
	.minimap-window {
		position: relative;
		display: flex;
		flex-direction: column;
		width: 100vw;
		height: 100vh;
		background: #0f0f1a;
		user-select: none;
		font-family: ui-monospace, 'SF Mono', 'Cascadia Code', 'Fira Code', monospace;
		border-radius: 6px;
		overflow: hidden;
	}

	.drag-frame {
		position: absolute;
		background: transparent;
		z-index: 50;
		-webkit-app-region: drag;
	}

	.drag-top {
		top: 0;
		left: 0;
		right: 0;
		height: 6px;
		cursor: grab;
	}

	.drag-bottom {
		bottom: 0;
		left: 0;
		right: 0;
		height: 6px;
		cursor: grab;
	}

	.drag-left {
		top: 0;
		left: 0;
		bottom: 0;
		width: 6px;
		cursor: grab;
	}

	.drag-right {
		top: 0;
		right: 0;
		bottom: 0;
		width: 6px;
		cursor: grab;
	}

	.hover-controls {
		position: absolute;
		top: 4px;
		right: 4px;
		display: flex;
		gap: 2px;
		opacity: 0;
		transition: opacity 0.15s;
		z-index: 100;
		-webkit-app-region: no-drag;
	}

	.minimap-window:hover .hover-controls {
		opacity: 1;
	}

	.control-btn {
		background: rgba(15, 15, 26, 0.9);
		border: 1px solid #2d2d44;
		font-size: 12px;
		cursor: pointer;
		padding: 2px 6px;
		border-radius: 3px;
		color: #64748b;
		transition: all 0.15s;
	}

	.control-btn:hover {
		background: #1a1a2e;
		color: #94a3b8;
	}

	.control-btn.active {
		color: #22c55e;
	}

	.control-btn.close:hover {
		color: #ef4444;
		border-color: rgba(239, 68, 68, 0.3);
	}

	.content {
		flex: 1;
		min-height: 0;
		padding: 4px;
		overflow: hidden;
		display: flex;
		align-items: center;
		justify-content: center;
		container-type: size;
	}

	.aspect-container {
		/*
		 * This container maintains the main app's aspect ratio.
		 * Uses object-fit: contain behavior via aspect-ratio + max constraints.
		 * The width: 100cqw gives us a starting width, then aspect-ratio determines height,
		 * but max-height: 100cqh caps it, and when it caps, aspect-ratio shrinks width too.
		 */
		width: 100cqw;
		max-width: 100cqw;
		max-height: 100cqh;
		/* aspect-ratio is set via inline style */
	}

	.empty {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		color: #64748b;
		font-size: 12px;
	}

	.split-container {
		display: flex;
		gap: 1px;
		width: 100%;
		height: 100%;
		background: #1e1e2e;
	}

	.split-container.horizontal {
		flex-direction: row;
	}

	.split-container.vertical {
		flex-direction: column;
	}

	.split-child {
		min-width: 0;
		min-height: 0;
		overflow: hidden;
	}

	.pane-thumb {
		width: 100%;
		height: 100%;
		border: none;
		border-radius: 0;
		padding: 0;
		background: #0a0a0f;
		cursor: pointer;
		-webkit-app-region: no-drag;
		transition: opacity 0.15s;
		overflow: hidden;
	}

	.pane-thumb:hover {
		opacity: 0.85;
	}

	.pane-thumb img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		pointer-events: none;
		-webkit-user-drag: none;
		display: block;
	}

	.terminal-thumb {
		background: #0a0a0f;
	}

	.webview-thumb {
		background: #1a1a2e;
	}

	.placeholder {
		width: 100%;
		height: 100%;
		background: #0a0a0f;
	}

	.webview-placeholder {
		width: 100%;
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 4px;
		background: linear-gradient(135deg, #1a1a2e 0%, #0f0f1a 100%);
		color: #64748b;
	}

	.webview-icon {
		font-size: 16px;
		color: #3b82f6;
	}

	.webview-url {
		font-size: 8px;
		max-width: 90%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		text-align: center;
	}
</style>
