<script lang="ts">
	import { onMount, onDestroy } from 'svelte';

	interface MenuItem {
		label: string;
		icon?: string;
		action: () => void;
		disabled?: boolean;
	}

	interface Props {
		x: number;
		y: number;
		items: MenuItem[];
		onClose: () => void;
	}

	let { x, y, items, onClose }: Props = $props();

	let menuEl: HTMLDivElement;

	onMount(() => {
		// Adjust position if menu would go off-screen
		if (menuEl) {
			const rect = menuEl.getBoundingClientRect();
			const viewportWidth = window.innerWidth;
			const viewportHeight = window.innerHeight;

			if (x + rect.width > viewportWidth) {
				x = viewportWidth - rect.width - 8;
			}
			if (y + rect.height > viewportHeight) {
				y = viewportHeight - rect.height - 8;
			}
		}

		// Close on click outside
		const handleClickOutside = (e: MouseEvent) => {
			if (menuEl && !menuEl.contains(e.target as Node)) {
				onClose();
			}
		};

		// Close on escape
		const handleKeydown = (e: KeyboardEvent) => {
			if (e.key === 'Escape') {
				onClose();
			}
		};

		document.addEventListener('mousedown', handleClickOutside);
		document.addEventListener('keydown', handleKeydown);

		return () => {
			document.removeEventListener('mousedown', handleClickOutside);
			document.removeEventListener('keydown', handleKeydown);
		};
	});

	function handleItemClick(item: MenuItem) {
		if (!item.disabled) {
			item.action();
			onClose();
		}
	}
</script>

<div
	class="context-menu"
	bind:this={menuEl}
	style:left="{x}px"
	style:top="{y}px"
>
	{#each items as item}
		<button
			class="menu-item"
			class:disabled={item.disabled}
			onclick={() => handleItemClick(item)}
		>
			{#if item.icon}
				<span class="icon">{item.icon}</span>
			{/if}
			<span class="label">{item.label}</span>
		</button>
	{/each}
</div>

<style>
	.context-menu {
		position: fixed;
		z-index: 10000;
		background: #1a1a2e;
		border: 1px solid #2d2d44;
		border-radius: 6px;
		padding: 4px;
		min-width: 180px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
	}

	.menu-item {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 8px 12px;
		background: none;
		border: none;
		border-radius: 4px;
		color: #e2e8f0;
		font-size: 13px;
		font-family: inherit;
		text-align: left;
		cursor: pointer;
		transition: background 0.1s;
	}

	.menu-item:hover:not(.disabled) {
		background: rgba(255, 255, 255, 0.1);
	}

	.menu-item.disabled {
		color: #64748b;
		cursor: not-allowed;
	}

	.icon {
		font-size: 14px;
		width: 18px;
		text-align: center;
	}

	.label {
		flex: 1;
	}
</style>
