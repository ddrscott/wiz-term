<script lang="ts">
	interface Props {
		/** Called with the new absolute width during resize */
		onWidthChange: (newWidth: number) => void;
		/** Returns the current width at the start of a drag */
		getCurrentWidth: () => number;
		/** Minimum width constraint */
		minWidth?: number;
		onResizeEnd?: () => void;
		disabled?: boolean;
	}

	let { onWidthChange, getCurrentWidth, minWidth = 200, onResizeEnd, disabled = false }: Props = $props();

	let isResizing = $state(false);

	function handleMouseDown(e: MouseEvent) {
		if (disabled) return;
		e.preventDefault();
		isResizing = true;

		// Capture the start position and width at drag start
		const startX = e.clientX;
		const startWidth = getCurrentWidth();

		function onMouseMove(e: MouseEvent) {
			const delta = e.clientX - startX;
			const newWidth = Math.max(minWidth, startWidth + delta);
			onWidthChange(newWidth);
		}

		function onMouseUp() {
			isResizing = false;
			window.removeEventListener('mousemove', onMouseMove);
			window.removeEventListener('mouseup', onMouseUp);
			document.body.style.cursor = '';
			document.body.style.userSelect = '';
			onResizeEnd?.();
		}

		// Prevent text selection while dragging
		document.body.style.cursor = 'col-resize';
		document.body.style.userSelect = 'none';

		window.addEventListener('mousemove', onMouseMove);
		window.addEventListener('mouseup', onMouseUp);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="resize-handle"
	class:active={isResizing}
	class:disabled
	onmousedown={handleMouseDown}
></div>

<style>
	.resize-handle {
		position: absolute;
		top: 0;
		right: -4px;
		width: 8px;
		height: 100%;
		cursor: col-resize;
		z-index: 10;
	}

	.resize-handle:hover,
	.resize-handle.active {
		background: rgba(59, 130, 246, 0.5);
	}

	.resize-handle.disabled {
		cursor: default;
		pointer-events: none;
	}
</style>
