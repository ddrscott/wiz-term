<script lang="ts">
	import type { SplitDirection } from '$lib/types/terminal';

	interface Props {
		direction: SplitDirection;
		onResize: (delta: number) => void;
	}

	let { direction, onResize }: Props = $props();

	let isResizing = $state(false);

	function handleMouseDown(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		isResizing = true;

		const startPos = direction === 'horizontal' ? e.clientX : e.clientY;
		let lastPos = startPos;

		function onMouseMove(e: MouseEvent) {
			const currentPos = direction === 'horizontal' ? e.clientX : e.clientY;
			const delta = currentPos - lastPos;
			lastPos = currentPos;
			if (delta !== 0) {
				onResize(delta);
			}
		}

		function onMouseUp() {
			isResizing = false;
			window.removeEventListener('mousemove', onMouseMove);
			window.removeEventListener('mouseup', onMouseUp);
			document.body.style.cursor = '';
			document.body.style.userSelect = '';
		}

		// Prevent text selection while dragging
		document.body.style.cursor = direction === 'horizontal' ? 'col-resize' : 'row-resize';
		document.body.style.userSelect = 'none';

		window.addEventListener('mousemove', onMouseMove);
		window.addEventListener('mouseup', onMouseUp);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="split-resizer"
	class:horizontal={direction === 'horizontal'}
	class:vertical={direction === 'vertical'}
	class:active={isResizing}
	onmousedown={handleMouseDown}
></div>

<style>
	.split-resizer {
		flex-shrink: 0;
		background: #1e1e2e;
		transition: background 0.15s;
	}

	.split-resizer.horizontal {
		width: 4px;
		cursor: col-resize;
	}

	.split-resizer.vertical {
		height: 4px;
		cursor: row-resize;
	}

	.split-resizer:hover,
	.split-resizer.active {
		background: rgba(59, 130, 246, 0.5);
	}
</style>
