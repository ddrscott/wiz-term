<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';

	interface Bounds {
		x: number;
		y: number;
		width: number;
		height: number;
	}

	interface Props {
		nodeId: string;
		url: string;
		title?: string;
		bounds?: Bounds; // Explicit bounds from parent for precise positioning
		onClose?: () => void;
		onTitleChange?: (title: string) => void;
		onUrlChange?: (url: string) => void;
		onFocus?: (nodeId: string) => void;
		onWidthChange?: (width: number) => void;
	}

	let { nodeId, url, title, bounds, onClose, onTitleChange, onUrlChange, onFocus, onWidthChange }: Props = $props();

	// Size presets for webview width
	const SIZE_PRESETS = {
		s: 320,
		m: 640,
		xl: 800
	} as const;

	function applyPreset(preset: keyof typeof SIZE_PRESETS) {
		onWidthChange?.(SIZE_PRESETS[preset]);
	}

	let containerEl: HTMLDivElement;
	let inputUrl = $state(url);
	let isLoading = $state(true);
	let webviewReady = false;
	let webviewId = `browser-${nodeId}`;

	// Derive bounds values for reactivity (all four values to track position AND size)
	let boundsX = $derived(bounds?.x ?? 0);
	let boundsY = $derived(bounds?.y ?? 0);
	let boundsWidth = $derived(bounds?.width ?? 0);
	let boundsHeight = $derived(bounds?.height ?? 0);

	// Update input when url prop changes
	$effect(() => {
		inputUrl = url;
	});

	// Update webview URL when url prop changes
	$effect(() => {
		if (webviewReady && url) {
			navigateWebview(url);
		}
	});

	// Update webview position when bounds change
	// Use derived values to ensure reactivity - track ALL bounds (x, y, width, height)
	$effect(() => {
		// Access all derived values to track changes - this makes the effect reactive to them
		const x = boundsX;
		const y = boundsY;
		const w = boundsWidth;
		const h = boundsHeight;

		if (webviewReady && w > 0 && h > 0) {
			console.log('Bounds changed, updating webview:', { x, y, width: w, height: h });
			// Pass values directly to avoid closure issues
			updateWebviewPositionWithBounds(x, y, w, h);
		}
	});

	onMount(async () => {
		// Wait for container to have proper dimensions before creating webview
		await waitForContainerSize();
		await createWebview();
	});

	async function waitForContainerSize(): Promise<void> {
		return new Promise((resolve) => {
			const checkSize = () => {
				if (containerEl) {
					const rect = containerEl.getBoundingClientRect();
					if (rect.width > 10 && rect.height > 10) {
						console.log('Container ready:', rect.width, 'x', rect.height);
						resolve();
						return;
					}
				}
				// Check again on next frame
				requestAnimationFrame(checkSize);
			};
			checkSize();
		});
	}

	onDestroy(async () => {
		await destroyWebview();
	});

	async function createWebview() {
		if (!containerEl) return;

		try {
			// Use explicit bounds if provided, otherwise measure container
			const rect = containerEl.getBoundingClientRect();

			// Calculate webview dimensions
			// Use the full lane width from derived bounds, but position at container's location
			const x = rect.left;
			const y = rect.top;
			const width = boundsWidth > 0 ? boundsWidth : rect.width;
			const height = rect.height;

			console.log('Creating webview via Rust backend:', webviewId, 'at', {
				x, y, width, height,
				boundsWidth: bounds?.width,
				rectWidth: rect.width
			});

			// Create webview via Rust command
			await invoke('create_webview', {
				id: webviewId,
				url: url,
				x,
				y,
				width,
				height
			});

			console.log('Webview created successfully:', webviewId);
			isLoading = false;
			webviewReady = true;

			// Start observing position changes
			startPositionObserver();
		} catch (e) {
			console.error('Failed to create webview:', e);
			isLoading = false;
		}
	}

	async function destroyWebview() {
		stopPositionObserver();
		if (webviewReady) {
			try {
				await invoke('close_webview', { id: webviewId });
				console.log('Webview closed:', webviewId);
			} catch (e) {
				console.error('Failed to close webview:', e);
			}
			webviewReady = false;
		}
	}

	async function navigateWebview(newUrl: string) {
		if (webviewReady) {
			try {
				await invoke('navigate_webview', { id: webviewId, url: newUrl });
			} catch (e) {
				console.error('Failed to navigate:', e);
			}
		}
	}

	let resizeObserver: ResizeObserver | null = null;
	let scrollContainer: HTMLElement | null = null;
	let animationFrameId: number | null = null;

	function startPositionObserver() {
		if (!containerEl) return;

		console.log('Starting position observer for webview:', webviewId);

		// Use ResizeObserver for size changes
		resizeObserver = new ResizeObserver(() => {
			requestPositionUpdate();
		});
		resizeObserver.observe(containerEl);

		// Find the scrolling parent (lanes-container) and listen to scroll events
		scrollContainer = containerEl.closest('.lanes-container');
		if (scrollContainer) {
			console.log('Found scroll container:', scrollContainer.className);
			scrollContainer.addEventListener('scroll', requestPositionUpdate, { passive: true });
		}

		// Also listen to window scroll/resize
		window.addEventListener('scroll', requestPositionUpdate, { capture: true, passive: true });
		window.addEventListener('resize', requestPositionUpdate);

		// Initial position update
		updateWebviewPosition();
	}

	function stopPositionObserver() {
		resizeObserver?.disconnect();
		resizeObserver = null;
		if (scrollContainer) {
			scrollContainer.removeEventListener('scroll', requestPositionUpdate);
			scrollContainer = null;
		}
		window.removeEventListener('scroll', requestPositionUpdate, true);
		window.removeEventListener('resize', requestPositionUpdate);
		if (animationFrameId !== null) {
			cancelAnimationFrame(animationFrameId);
			animationFrameId = null;
		}
	}

	function requestPositionUpdate() {
		// Use requestAnimationFrame to batch position updates
		if (animationFrameId === null) {
			animationFrameId = requestAnimationFrame(() => {
				animationFrameId = null;
				updateWebviewPosition();
			});
		}
	}

	async function updateWebviewPosition() {
		// Use current derived values
		updateWebviewPositionWithBounds(boundsX, boundsY, boundsWidth, boundsHeight);
	}

	async function updateWebviewPositionWithBounds(
		explicitX: number,
		explicitY: number,
		explicitWidth: number,
		explicitHeight: number
	) {
		if (!webviewReady || !containerEl) return;

		const rect = containerEl.getBoundingClientRect();

		// Use explicit bounds for position and size
		// The x/y from bounds is relative to the lanes container, but we need screen coordinates
		// So we still use rect.left/top for position, but the bounds trigger the update
		const width = explicitWidth > 0 ? explicitWidth : rect.width;
		const height = rect.height;

		// Only update if dimensions are valid
		if (width <= 0 || height <= 0) return;

		try {
			await invoke('update_webview', {
				id: webviewId,
				x: rect.left,
				y: rect.top,
				width,
				height
			});
		} catch (e) {
			// Webview might be closing
			console.error('Failed to update webview position:', e);
		}
	}

	function handleNavigate() {
		let newUrl = inputUrl.trim();
		if (!newUrl) return;

		// Add protocol if missing
		if (!newUrl.startsWith('http://') && !newUrl.startsWith('https://')) {
			if (newUrl.includes('.') && !newUrl.includes(' ')) {
				newUrl = 'https://' + newUrl;
			} else {
				newUrl = `https://www.google.com/search?q=${encodeURIComponent(newUrl)}`;
			}
		}

		inputUrl = newUrl;
		onUrlChange?.(newUrl);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			handleNavigate();
		}
	}

	async function refresh() {
		if (webviewReady) {
			try {
				await invoke('eval_webview', { id: webviewId, script: 'window.location.reload()' });
			} catch (e) {
				console.error('Failed to refresh:', e);
			}
		}
	}

	async function goBack() {
		if (webviewReady) {
			try {
				await invoke('eval_webview', { id: webviewId, script: 'window.history.back()' });
			} catch (e) {
				console.error('Failed to go back:', e);
			}
		}
	}

	async function goForward() {
		if (webviewReady) {
			try {
				await invoke('eval_webview', { id: webviewId, script: 'window.history.forward()' });
			} catch (e) {
				console.error('Failed to go forward:', e);
			}
		}
	}

	async function openInBrowser() {
		const { open } = await import('@tauri-apps/plugin-shell');
		await open(inputUrl);
	}

	function handleFocus() {
		onFocus?.(nodeId);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="webview-pane" onfocusin={handleFocus}>
	<div class="webview-header">
		<div class="nav-buttons">
			<button class="nav-btn" onclick={goBack} title="Back">‹</button>
			<button class="nav-btn" onclick={goForward} title="Forward">›</button>
			<button class="nav-btn" onclick={refresh} title="Refresh">↻</button>
		</div>

		<input
			class="url-input"
			type="text"
			bind:value={inputUrl}
			onkeydown={handleKeydown}
			placeholder="URL or search..."
		/>

		<button class="action-btn" onclick={openInBrowser} title="Open in browser">↗</button>
		<div class="size-presets">
			<button class="size-btn" onclick={() => applyPreset('s')} title="Small (320px)">s</button>
			<button class="size-btn" onclick={() => applyPreset('m')} title="Medium (640px)">m</button>
			<button class="size-btn" onclick={() => applyPreset('xl')} title="Extra Large (800px)">xl</button>
		</div>
		<button class="close-btn" onclick={onClose} title="Close">×</button>
	</div>

	<!-- Container for native webview positioning -->
	<div class="webview-container" bind:this={containerEl}>
		{#if isLoading}
			<div class="loading-overlay">
				<div class="spinner"></div>
				<span>Loading webview...</span>
			</div>
		{/if}
	</div>
</div>

<style>
	.webview-pane {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		background: #0f0f1a;
		overflow: hidden;
	}

	.webview-header {
		padding: 4px 8px;
		background: #0f0f1a;
		border-bottom: 1px solid #1e1e2e;
		display: flex;
		align-items: center;
		flex-shrink: 0;
		gap: 6px;
	}

	.nav-buttons {
		display: flex;
		gap: 1px;
	}

	.nav-btn {
		padding: 0 3px;
		background: none;
		border: none;
		color: #4b5563;
		cursor: pointer;
		font-size: 12px;
		line-height: 1;
		transition: color 0.15s;
	}

	.nav-btn:hover {
		color: #94a3b8;
	}

	.url-input {
		flex: 1;
		min-width: 0;
		padding: 0;
		background: transparent;
		border: none;
		color: #94a3b8;
		font-size: 13px;
		font-family: ui-monospace, 'SF Mono', 'Cascadia Code', 'Fira Code', monospace;
		outline: none;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.url-input:focus {
		color: #e2e8f0;
	}

	.url-input::placeholder {
		color: #4b5563;
	}

	.action-btn {
		background: none;
		border: none;
		color: #4b5563;
		cursor: pointer;
		font-size: 11px;
		padding: 0 2px;
		transition: color 0.15s;
	}

	.action-btn:hover {
		color: #94a3b8;
	}

	.size-presets {
		display: flex;
		gap: 2px;
	}

	.size-btn {
		background: none;
		border: 1px solid transparent;
		color: #4b5563;
		cursor: pointer;
		font-size: 11px;
		font-weight: 500;
		line-height: 1;
		padding: 2px 4px;
		border-radius: 2px;
		transition: all 0.15s;
		font-family: ui-monospace, 'SF Mono', monospace;
		text-transform: uppercase;
	}

	.size-btn:hover {
		color: #94a3b8;
		border-color: #374151;
		background: rgba(255, 255, 255, 0.05);
	}

	.close-btn {
		background: none;
		border: none;
		color: #64748b;
		cursor: pointer;
		font-size: 14px;
		line-height: 1;
		padding: 0 2px;
		border-radius: 2px;
		transition: all 0.15s;
	}

	.close-btn:hover {
		color: #ef4444;
		background: rgba(239, 68, 68, 0.1);
	}

	.webview-container {
		flex: 1;
		width: 100%;
		position: relative;
		min-height: 0;
		/* This is where the native webview will be positioned */
	}

	.loading-overlay {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 12px;
		background: #0f0f1a;
		color: #64748b;
		font-size: 13px;
	}

	.spinner {
		width: 24px;
		height: 24px;
		border: 2px solid #1e1e2e;
		border-top-color: #3b82f6;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
