<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { terminalBounds } from '$lib/stores/terminalBounds';

	interface Props {
		nodeId: string;
		sessionId?: string; // Optional - terminals have sessionId, webviews don't
		containerEl: HTMLElement | null; // Reference to the lanes container for relative positioning
		focusedNodeId: string | null;
		onFocus: (nodeId: string) => void;
		// Counter to force bounds recalculation when column widths change
		boundsGeneration?: number;
	}

	let { nodeId, sessionId, containerEl, focusedNodeId, onFocus, boundsGeneration = 0 }: Props = $props();

	let slotEl: HTMLDivElement;
	let resizeObserver: ResizeObserver | null = null;

	// Track last bounds to avoid unnecessary updates (prevents ResizeObserver feedback loop)
	let lastBounds = { x: 0, y: 0, width: 0, height: 0 };

	// React to boundsGeneration changes to force bounds recalculation
	$effect(() => {
		// Subscribe to boundsGeneration changes
		const _ = boundsGeneration;
		// Delay to allow DOM to update first
		requestAnimationFrame(() => {
			updateBounds();
		});
	});

	function updateBounds() {
		if (!slotEl || !containerEl) return;

		const slotRect = slotEl.getBoundingClientRect();
		const containerRect = containerEl.getBoundingClientRect();

		const newBounds = {
			x: slotRect.left - containerRect.left + containerEl.scrollLeft,
			y: slotRect.top - containerRect.top + containerEl.scrollTop,
			width: slotRect.width,
			height: slotRect.height
		};

		// Only update if bounds actually changed (prevents feedback loop)
		if (
			Math.abs(newBounds.x - lastBounds.x) > 0.5 ||
			Math.abs(newBounds.y - lastBounds.y) > 0.5 ||
			Math.abs(newBounds.width - lastBounds.width) > 0.5 ||
			Math.abs(newBounds.height - lastBounds.height) > 0.5
		) {
			lastBounds = newBounds;
			terminalBounds.updateBounds(nodeId, newBounds);
		}
	}

	onMount(() => {
		// Initial bounds update after layout settles
		requestAnimationFrame(() => {
			updateBounds();
		});

		// Watch for size changes - use requestAnimationFrame to debounce
		resizeObserver = new ResizeObserver(() => {
			requestAnimationFrame(updateBounds);
		});
		resizeObserver.observe(slotEl);

		// Also update on scroll (in case container scrolls)
		containerEl?.addEventListener('scroll', updateBounds);
	});

	onDestroy(() => {
		resizeObserver?.disconnect();
		containerEl?.removeEventListener('scroll', updateBounds);
		terminalBounds.removeBounds(nodeId);
	});

	function handleClick() {
		onFocus(nodeId);
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="layout-slot"
	class:focused={focusedNodeId === nodeId}
	bind:this={slotEl}
	onclick={handleClick}
>
	<!-- Empty slot - terminal renders in TerminalRegistry and positions over this -->
</div>

<style>
	.layout-slot {
		flex: 1;
		min-height: 0;
		min-width: 0;
		background: var(--terminal-bg);
	}

	.layout-slot.focused {
		outline: 1px solid #3b82f6;
		outline-offset: -1px;
	}
</style>
