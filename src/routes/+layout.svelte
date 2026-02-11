<script lang="ts">
	import '../app.css';
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/stores';
	import { terminalActions } from '$lib/stores/terminal';
	import { minimapStore } from '$lib/stores/minimapStore';
	import { contextMenuStore } from '$lib/stores/contextMenu';
	import ContextMenu from '$lib/components/shared/ContextMenu.svelte';
	import SettingsPanel from '$lib/components/shared/SettingsPanel.svelte';

	let { children } = $props();
	let unlistenToggle: (() => void) | null = null;
	let unlistenPin: (() => void) | null = null;
	let unlistenReset: (() => void) | null = null;

	let currentPath = $derived($page.url.pathname);
	let isStandalonePage = $derived(currentPath === '/minimap');

	// Settings panel state
	let showSettings = $state(false);

	// Dynamic import state - only load in browser after mount
	let TerminalLanes: typeof import('$lib/components/terminal/TerminalLanes.svelte').default | null = $state(null);

	// Tauri window API for manual drag
	let startDragging: (() => Promise<void>) | null = null;

	onMount(async () => {
		// Dynamic import xterm.js components only in browser
		const module = await import('$lib/components/terminal/TerminalLanes.svelte');
		TerminalLanes = module.default;

		// Import Tauri window API for manual drag
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		const currentWindow = getCurrentWindow();
		startDragging = () => currentWindow.startDragging();

		// Listen for native menu events
		const { listen } = await import('@tauri-apps/api/event');
		unlistenToggle = await listen('menu-toggle-minimap', () => {
			minimapStore.toggleWindow();
		});
		unlistenPin = await listen('menu-pin-minimap', () => {
			minimapStore.togglePinned();
		});
		unlistenReset = await listen('menu-reset-minimap', () => {
			minimapStore.resetPosition();
		});
	});

	onDestroy(() => {
		unlistenToggle?.();
		unlistenPin?.();
		unlistenReset?.();
	});

	function handleNewTerminal() {
		terminalActions.requestNewTerminal();
	}

	function handleHeaderMouseDown(e: MouseEvent) {
		// Only drag on left click and if not clicking a button
		if (e.button === 0 && startDragging && !(e.target as HTMLElement).closest('button')) {
			startDragging();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		// Cmd/Ctrl+N: New terminal
		if ((e.metaKey || e.ctrlKey) && e.key === 'n') {
			e.preventDefault();
			handleNewTerminal();
		}
		// Cmd/Ctrl+Shift+M: Toggle minimap
		if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'm') {
			e.preventDefault();
			minimapStore.toggleWindow();
		}
		// Cmd/Ctrl+,: Open settings
		if ((e.metaKey || e.ctrlKey) && e.key === ',') {
			e.preventDefault();
			showSettings = true;
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Standalone pages (like minimap) render without app shell -->
{#if isStandalonePage}
	{@render children()}
{:else}
	<div class="terminal-app">
		<header class="terminal-header" onmousedown={handleHeaderMouseDown}>
			<div class="header-left">
				<span class="prompt">$</span>
				<span class="app-title">wiz-term</span>
			</div>
			<div class="header-actions">
				<button class="action-btn" onclick={handleNewTerminal} title="New terminal (Cmd+N)">
					+
				</button>
				<button class="action-btn" onclick={() => minimapStore.toggleWindow()} title="Toggle minimap (Cmd+Shift+M)">
					⊞
				</button>
				<button class="action-btn" onclick={() => showSettings = true} title="Settings (Cmd+,)">
					⚙
				</button>
			</div>
		</header>

		<main class="terminal-main">
			{#if TerminalLanes}
				<TerminalLanes visible={true} />
			{/if}
		</main>
	</div>
{/if}

<!-- Global context menu - rendered at top level to escape transform containing blocks -->
{#if $contextMenuStore}
	<ContextMenu
		x={$contextMenuStore.x}
		y={$contextMenuStore.y}
		items={$contextMenuStore.items}
		onClose={() => contextMenuStore.close()}
	/>
{/if}

<!-- Settings panel -->
{#if showSettings}
	<SettingsPanel onClose={() => showSettings = false} />
{/if}

<style>
	.terminal-app {
		height: 100vh;
		display: flex;
		flex-direction: column;
		background: #0a0a0f;
		color: #e2e8f0;
		overflow: hidden;
		/* Prevent any overscroll effects from bubbling */
		overscroll-behavior: none;
	}

	.terminal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		/* Left padding for macOS traffic lights */
		padding: 0 12px 0 78px;
		height: 38px;
		background: #0f0f1a;
		border-bottom: 1px solid #1e1e2e;
		flex-shrink: 0;
		/* Prevent text selection, use default cursor for titlebar feel */
		user-select: none;
		-webkit-user-select: none;
		cursor: default;
	}

	.header-left {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 12px;
	}

	.prompt {
		color: #22c55e;
		font-weight: bold;
	}

	.app-title {
		color: #64748b;
		font-weight: 500;
	}

	.header-actions {
		display: flex;
		gap: 2px;
	}

	.action-btn {
		width: 28px;
		height: 28px;
		padding: 0;
		background: none;
		border: 1px solid transparent;
		border-radius: 4px;
		color: #64748b;
		cursor: pointer;
		font-family: inherit;
		font-size: 14px;
		line-height: 1;
		transition: all 0.15s;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.action-btn:hover {
		color: #e2e8f0;
		background: rgba(255, 255, 255, 0.1);
		border-color: #2d2d44;
	}

	.terminal-main {
		flex: 1;
		min-height: 0;
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}
</style>
