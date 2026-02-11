<script lang="ts">
	import type { TerminalNode, TerminalSession } from '$lib/types/terminal';
	import TerminalLane from './TerminalLane.svelte';

	interface Props {
		node: TerminalNode;
		session: TerminalSession | undefined;
		visible: boolean;
		focusedNodeId: string | null;
		onClose: (sessionId: string) => void;
		onFocus: (nodeId: string) => void;
	}

	let {
		node,
		session,
		visible,
		focusedNodeId,
		onClose,
		onFocus
	}: Props = $props();

	function handlePaneClick() {
		onFocus(node.id);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
	class="terminal-pane"
	class:focused={focusedNodeId === node.id}
	onclick={handlePaneClick}
>
	{#if session}
		<TerminalLane
			{session}
			nodeId={node.id}
			{visible}
			onClose={() => onClose(session.id)}
		/>
	{:else}
		<div class="missing-session">
			<span>Session not found</span>
			<button onclick={() => onClose(node.sessionId)}>Close</button>
		</div>
	{/if}
</div>

<style>
	.terminal-pane {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		width: 100%;
		min-height: 0;
		min-width: 0;
		user-select: none;
	}

	.terminal-pane.focused {
		outline: 1px solid #3b82f6;
		outline-offset: -1px;
	}

	.missing-session {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		gap: 12px;
		color: #64748b;
		background: var(--terminal-bg);
	}

	.missing-session button {
		background: rgba(239, 68, 68, 0.2);
		border: 1px solid rgba(239, 68, 68, 0.3);
		color: #ef4444;
		padding: 6px 12px;
		border-radius: 4px;
		cursor: pointer;
	}
</style>
