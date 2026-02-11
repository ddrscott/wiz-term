<script lang="ts">
	import '../app.css';
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/stores';
	import { terminalActions } from '$lib/stores/terminal';
	import { minimapStore } from '$lib/stores/minimapStore';

	let { children } = $props();
	let unlistenToggle: (() => void) | null = null;
	let unlistenPin: (() => void) | null = null;
	let unlistenReset: (() => void) | null = null;

	let currentPath = $derived($page.url.pathname);
	let isStandalonePage = $derived(currentPath === '/minimap');

	// Dynamic import state - only load in browser after mount
	let TerminalLanes: typeof import('$lib/components/terminal/TerminalLanes.svelte').default | null = $state(null);

	onMount(async () => {
		// Dynamic import xterm.js components only in browser
		const module = await import('$lib/components/terminal/TerminalLanes.svelte');
		TerminalLanes = module.default;

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
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Standalone pages (like minimap) render without app shell -->
{#if isStandalonePage}
	{@render children()}
{:else}
	<div class="terminal-app">
		<header class="terminal-header">
			<div class="header-left">
				<span class="prompt">$</span>
				<span class="app-title">wiz-term</span>
				<span class="cursor">_</span>
			</div>
			<div class="header-actions">
				<button class="action-btn" onclick={handleNewTerminal} title="New terminal (Cmd+N)">
					[+new]
				</button>
				<button class="action-btn" onclick={() => minimapStore.toggleWindow()} title="Toggle minimap (Cmd+Shift+M)">
					[minimap]
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
		padding: 10px 16px;
		background: #0f0f1a;
		border-bottom: 1px solid #1e1e2e;
		flex-shrink: 0;
	}

	.header-left {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.prompt {
		color: #22c55e;
		font-weight: bold;
	}

	.app-title {
		color: #e2e8f0;
		font-weight: 600;
	}

	.cursor {
		color: #22c55e;
		animation: blink 1s step-end infinite;
	}

	@keyframes blink {
		50% { opacity: 0; }
	}

	.header-actions {
		display: flex;
		gap: 8px;
	}

	.action-btn {
		padding: 4px 8px;
		background: none;
		border: none;
		color: #64748b;
		cursor: pointer;
		font-family: inherit;
		font-size: inherit;
		transition: color 0.15s;
	}

	.action-btn:hover {
		color: #22c55e;
	}

	.terminal-main {
		flex: 1;
		min-height: 0;
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}
</style>
