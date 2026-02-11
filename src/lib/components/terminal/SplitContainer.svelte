<script lang="ts">
	import type { LayoutNode } from '$lib/types/terminal';
	import LayoutSlot from './LayoutSlot.svelte';
	import SplitResizer from './SplitResizer.svelte';
	import SplitContainer from './SplitContainer.svelte';
	import ResizeHandle from '$lib/components/shared/ResizeHandle.svelte';

	interface Props {
		node: LayoutNode;
		lanesContainerEl: HTMLElement | null; // Reference to root container for bounds calculation
		focusedNodeId: string | null;
		onResize: (nodeId: string, sizes: number[]) => void;
		onFocus: (nodeId: string) => void;
		// For root-level horizontal scrolling
		isRoot?: boolean;
		columnWidths?: Map<string, number>;
		onColumnWidthChange?: (nodeId: string, width: number) => void;
		// Counter to force bounds recalculation
		boundsGeneration?: number;
	}

	const DEFAULT_COLUMN_WIDTH = 640; // 80 columns at default font
	const MIN_COLUMN_WIDTH = 200;

	let {
		node,
		lanesContainerEl,
		focusedNodeId,
		onResize,
		onFocus,
		isRoot = false,
		columnWidths = new Map(),
		onColumnWidthChange,
		boundsGeneration = 0
	}: Props = $props();

	let containerEl = $state<HTMLDivElement | null>(null);

	// Get column width for a child node
	function getColumnWidth(childId: string): number {
		return columnWidths.get(childId) ?? DEFAULT_COLUMN_WIDTH;
	}

	function handleResize(index: number, delta: number) {
		if (node.type !== 'split' || !containerEl) return;

		const isVertical = node.direction === 'vertical';
		const containerSize = isVertical ? containerEl.offsetHeight : containerEl.offsetWidth;

		if (containerSize === 0) return;

		// Convert delta to percentage
		const deltaPercent = (delta / containerSize) * 100;

		// Get current sizes
		const sizes = [...node.sizes];

		// Adjust sizes for the two children around the resizer
		const minSize = 10; // Minimum 10%

		const newSize1 = sizes[index] + deltaPercent;
		const newSize2 = sizes[index + 1] - deltaPercent;

		// Enforce minimum sizes
		if (newSize1 >= minSize && newSize2 >= minSize) {
			sizes[index] = newSize1;
			sizes[index + 1] = newSize2;
			onResize(node.id, sizes);
		}
	}
</script>

{#if node.type === 'terminal'}
	<!-- Terminal slot - fills available space -->
	<LayoutSlot
		nodeId={node.id}
		sessionId={node.sessionId}
		containerEl={lanesContainerEl}
		{focusedNodeId}
		{onFocus}
		{boundsGeneration}
	/>
{:else if node.type === 'webview'}
	<!-- Webview slot - fills available space -->
	<LayoutSlot
		nodeId={node.id}
		containerEl={lanesContainerEl}
		{focusedNodeId}
		{onFocus}
		{boundsGeneration}
	/>
{:else if isRoot && node.direction === 'horizontal'}
	<!-- Root-level horizontal split: fixed-width columns for horizontal scrolling -->
	<div class="split-container horizontal root-horizontal" bind:this={containerEl}>
		{#each node.children as child (child.id)}
			<div
				class="column-wrapper"
				style:width="{getColumnWidth(child.id)}px"
				style:min-width="{getColumnWidth(child.id)}px"
			>
				<SplitContainer
					node={child}
					{lanesContainerEl}
					{focusedNodeId}
					{onResize}
					{onFocus}
					{columnWidths}
					{onColumnWidthChange}
					{boundsGeneration}
				/>
				<ResizeHandle
					getCurrentWidth={() => getColumnWidth(child.id)}
					onWidthChange={(width) => onColumnWidthChange?.(child.id, width)}
					minWidth={MIN_COLUMN_WIDTH}
				/>
			</div>
		{/each}
	</div>
{:else if node.direction === 'horizontal'}
	<!-- Non-root horizontal split: also uses fixed-width columns for independent resizing -->
	<div class="split-container horizontal" bind:this={containerEl}>
		{#each node.children as child (child.id)}
			<div
				class="column-wrapper"
				style:width="{getColumnWidth(child.id)}px"
				style:min-width="{getColumnWidth(child.id)}px"
			>
				<SplitContainer
					node={child}
					{lanesContainerEl}
					{focusedNodeId}
					{onResize}
					{onFocus}
					{columnWidths}
					{onColumnWidthChange}
					{boundsGeneration}
				/>
				<ResizeHandle
					getCurrentWidth={() => getColumnWidth(child.id)}
					onWidthChange={(width) => onColumnWidthChange?.(child.id, width)}
					minWidth={MIN_COLUMN_WIDTH}
				/>
			</div>
		{/each}
	</div>
{:else}
	<!-- Vertical split container - uses percentage-based flex sizing -->
	<div
		class="split-container vertical"
		bind:this={containerEl}
	>
		{#each node.children as child, i (child.id)}
			<div class="split-child" style:flex-basis="{node.sizes[i]}%">
				<SplitContainer
					node={child}
					{lanesContainerEl}
					{focusedNodeId}
					{onResize}
					{onFocus}
					{columnWidths}
					{onColumnWidthChange}
					{boundsGeneration}
				/>
			</div>
			{#if i < node.children.length - 1}
				<SplitResizer
					direction={node.direction}
					onResize={(delta) => handleResize(i, delta)}
				/>
			{/if}
		{/each}
	</div>
{/if}

<style>
	.split-container {
		display: flex;
		min-height: 0;
		min-width: 0;
		height: 100%;
		user-select: none;
	}

	/* Root horizontal container allows content to determine width (enables horizontal scroll) */
	.split-container.root-horizontal {
		width: auto;
	}

	/* Non-root containers fill their parent */
	.split-container:not(.root-horizontal) {
		width: 100%;
	}

	.split-container.horizontal {
		flex-direction: row;
	}

	.split-container.vertical {
		flex-direction: column;
	}

	/* Fixed-width column wrapper for root-level horizontal scrolling */
	.column-wrapper {
		position: relative;
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		height: 100%;
		border-right: 2px solid #333333;
	}

	/* Keep right border on last lane too for visual consistency */

	.split-child {
		display: flex;
		min-height: 0;
		min-width: 0;
		overflow: hidden;
		/* Allow flex-basis percentage to work, but don't grow/shrink beyond it */
		flex-grow: 1;
		flex-shrink: 1;
	}

	/* Add visual separator between horizontal panes (non-root) */
	.split-container.horizontal:not(.root-horizontal) > .split-child:not(:last-child) {
		border-right: 2px solid #333333;
	}

	/* Add visual separator between vertical panes */
	.split-container.vertical > .split-child:not(:last-child) {
		border-bottom: 2px solid #333333;
	}
</style>
