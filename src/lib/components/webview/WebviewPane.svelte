<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';

	interface Props {
		nodeId: string;
		url: string;
		title?: string;
		onClose?: () => void;
		onTitleChange?: (title: string) => void;
		onUrlChange?: (url: string) => void;
		onFocus?: (nodeId: string) => void;
	}

	let { nodeId, url, title, onClose, onTitleChange, onUrlChange, onFocus }: Props = $props();

	let containerEl: HTMLDivElement;
	let inputUrl = $state(url);
	let isLoading = $state(true);
	let webviewReady = false;
	let webviewId = `browser-${nodeId}`;

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
			const rect = containerEl.getBoundingClientRect();

			console.log('Creating webview via Rust backend:', webviewId, 'at', {
				x: rect.left,
				y: rect.top,
				width: rect.width,
				height: rect.height
			});

			// Create webview via Rust command
			await invoke('create_webview', {
				id: webviewId,
				url: url,
				x: rect.left,
				y: rect.top,
				width: rect.width,
				height: rect.height
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
		if (!webviewReady || !containerEl) return;

		const rect = containerEl.getBoundingClientRect();
		const newWidth = Math.round(rect.width);
		const newHeight = Math.round(rect.height);

		// Only update if dimensions are valid
		if (newWidth <= 0 || newHeight <= 0) return;

		try {
			await invoke('update_webview', {
				id: webviewId,
				x: rect.left,
				y: rect.top,
				width: rect.width,
				height: rect.height
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
		z-index: 10;
	}

	.nav-buttons {
		display: flex;
		gap: 2px;
	}

	.nav-btn {
		width: 20px;
		height: 20px;
		padding: 0;
		background: none;
		border: none;
		color: #64748b;
		cursor: pointer;
		font-size: 14px;
		line-height: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: color 0.15s;
	}

	.nav-btn:hover {
		color: #e2e8f0;
	}

	.url-input {
		flex: 1;
		min-width: 0;
		padding: 3px 8px;
		background: #1a1a2e;
		border: 1px solid #2d2d44;
		border-radius: 3px;
		color: #94a3b8;
		font-size: 11px;
		font-family: ui-monospace, 'SF Mono', monospace;
		outline: none;
		transition: border-color 0.15s;
	}

	.url-input:focus {
		border-color: #3b82f6;
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
		font-size: 12px;
		padding: 2px 4px;
		transition: color 0.15s;
	}

	.action-btn:hover {
		color: #94a3b8;
	}

	.close-btn {
		background: none;
		border: none;
		color: #64748b;
		cursor: pointer;
		font-size: 14px;
		line-height: 1;
		padding: 2px 4px;
		border-radius: 3px;
		transition: all 0.15s;
	}

	.close-btn:hover {
		color: #ef4444;
		background: rgba(239, 68, 68, 0.1);
	}

	.webview-container {
		flex: 1;
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
