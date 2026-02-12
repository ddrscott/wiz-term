<script lang="ts">
	import { settings } from '$lib/stores/settings';
	import { TERMINAL_FONTS } from '$lib/types/terminal';

	interface Props {
		onClose: () => void;
		errorMessage?: string | null;
	}

	let { onClose, errorMessage = null }: Props = $props();

	// Local state bound to inputs
	let fontFamily = $state($settings.terminal.font_family);
	let fontSize = $state($settings.terminal.font_size);
	let useWebgl = $state($settings.terminal.use_webgl);
	let cursorBlink = $state($settings.terminal.cursor_blink);
	let scrollback = $state($settings.terminal.scrollback);
	let shellPath = $state($settings.terminal.shell_path || '/bin/zsh');

	// Custom font input (for fonts not in the list)
	let customFont = $state('');
	let showCustomFont = $state(!TERMINAL_FONTS.includes(fontFamily as any));

	$effect(() => {
		if (showCustomFont) {
			customFont = fontFamily;
		}
	});

	async function saveSettings() {
		const finalFont = showCustomFont ? customFont : fontFamily;
		await settings.updateTerminal({
			font_family: finalFont,
			font_size: fontSize,
			use_webgl: useWebgl,
			cursor_blink: cursorBlink,
			scrollback: scrollback,
			shell_path: shellPath
		});
		onClose();
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onClose();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onClose();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="settings-backdrop" onclick={handleBackdropClick}>
	<div class="settings-panel">
		<header class="settings-header">
			<h2>Settings</h2>
			<button class="close-btn" onclick={onClose}>×</button>
		</header>

		<div class="settings-content">
			{#if errorMessage}
				<div class="error-banner">
					<strong>Terminal failed to start</strong>
					<p>{errorMessage}</p>
					<p class="error-hint">Try configuring a different shell path below.</p>
				</div>
			{/if}

			<section class="settings-section">
				<h3>Shell</h3>

				<div class="setting-row">
					<label for="shell-path">
						<span>Shell Path</span>
						<span class="setting-hint">Full path to shell executable</span>
					</label>
					<input
						id="shell-path"
						type="text"
						bind:value={shellPath}
						placeholder="/bin/zsh"
						class="shell-input"
					/>
				</div>
			</section>

			<section class="settings-section">
				<h3>Font</h3>

				<div class="setting-row">
					<label for="font-select">Font Family</label>
					<div class="font-controls">
						{#if !showCustomFont}
							<select id="font-select" bind:value={fontFamily}>
								{#each TERMINAL_FONTS as font}
									<option value={font}>{font}</option>
								{/each}
							</select>
							<button class="text-btn" onclick={() => showCustomFont = true}>Custom...</button>
						{:else}
							<input
								type="text"
								bind:value={customFont}
								placeholder="Enter font name..."
								class="font-input"
							/>
							<button class="text-btn" onclick={() => { showCustomFont = false; fontFamily = 'SF Mono'; }}>Presets</button>
						{/if}
					</div>
				</div>

				<div class="setting-row">
					<label for="font-size">Font Size</label>
					<div class="size-controls">
						<input
							id="font-size"
							type="range"
							min="8"
							max="24"
							bind:value={fontSize}
						/>
						<span class="size-value">{fontSize}px</span>
					</div>
				</div>
			</section>

			<section class="settings-section">
				<h3>Rendering</h3>

				<div class="setting-row">
					<label for="use-webgl">
						<span>Renderer</span>
						<span class="setting-hint">Canvas may look sharper on Retina displays</span>
					</label>
					<select id="use-webgl" bind:value={useWebgl}>
						<option value={true}>WebGL (faster)</option>
						<option value={false}>Canvas (sharper)</option>
					</select>
				</div>
			</section>

			<section class="settings-section">
				<h3>Cursor</h3>

				<div class="setting-row">
					<label for="cursor-blink">Cursor Blink</label>
					<input id="cursor-blink" type="checkbox" bind:checked={cursorBlink} />
				</div>
			</section>

			<section class="settings-section">
				<h3>Scrollback</h3>

				<div class="setting-row">
					<label for="scrollback">Lines</label>
					<input
						id="scrollback"
						type="number"
						min="1000"
						max="100000"
						step="1000"
						bind:value={scrollback}
					/>
				</div>
			</section>
		</div>

		<footer class="settings-footer">
			<span class="restart-hint">Changes require restart for existing terminals</span>
			<div class="footer-buttons">
				<button class="cancel-btn" onclick={onClose}>Cancel</button>
				<button class="save-btn" onclick={saveSettings}>Save</button>
			</div>
		</footer>
	</div>
</div>

<style>
	.error-banner {
		background: rgba(239, 68, 68, 0.15);
		border: 1px solid rgba(239, 68, 68, 0.4);
		border-radius: 6px;
		padding: 12px;
		margin-bottom: 16px;
	}

	.error-banner strong {
		display: block;
		color: #ef4444;
		font-size: 13px;
		margin-bottom: 4px;
	}

	.error-banner p {
		margin: 0;
		font-size: 12px;
		color: #fca5a5;
		line-height: 1.4;
	}

	.error-banner .error-hint {
		margin-top: 8px;
		color: #94a3b8;
	}

	.settings-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
		backdrop-filter: blur(2px);
	}

	.settings-panel {
		background: #0f0f1a;
		border: 1px solid #2d2d44;
		border-radius: 8px;
		width: 400px;
		max-height: 80vh;
		display: flex;
		flex-direction: column;
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
	}

	.settings-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px;
		border-bottom: 1px solid #1e1e2e;
	}

	.settings-header h2 {
		margin: 0;
		font-size: 14px;
		font-weight: 600;
		color: #e2e8f0;
	}

	.close-btn {
		background: none;
		border: none;
		color: #64748b;
		font-size: 18px;
		cursor: pointer;
		padding: 4px 8px;
		border-radius: 4px;
		line-height: 1;
	}

	.close-btn:hover {
		color: #e2e8f0;
		background: rgba(255, 255, 255, 0.1);
	}

	.settings-content {
		flex: 1;
		overflow-y: auto;
		padding: 16px;
	}

	.settings-section {
		margin-bottom: 20px;
	}

	.settings-section:last-child {
		margin-bottom: 0;
	}

	.settings-section h3 {
		margin: 0 0 12px;
		font-size: 11px;
		font-weight: 600;
		color: #64748b;
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}

	.setting-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		margin-bottom: 12px;
	}

	.setting-row:last-child {
		margin-bottom: 0;
	}

	.setting-row label {
		font-size: 13px;
		color: #e2e8f0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.setting-hint {
		font-size: 11px;
		color: #64748b;
	}

	.setting-row select,
	.setting-row input[type="number"] {
		background: #1a1a2e;
		border: 1px solid #2d2d44;
		border-radius: 4px;
		padding: 6px 10px;
		color: #e2e8f0;
		font-size: 13px;
		font-family: inherit;
	}

	.setting-row select:focus,
	.setting-row input:focus {
		outline: none;
		border-color: #3b82f6;
	}

	.setting-row input[type="checkbox"] {
		width: 16px;
		height: 16px;
		accent-color: #22c55e;
	}

	.font-controls {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.font-controls select,
	.font-input {
		flex: 1;
		min-width: 140px;
	}

	.font-input,
	.shell-input {
		background: #1a1a2e;
		border: 1px solid #2d2d44;
		border-radius: 4px;
		padding: 6px 10px;
		color: #e2e8f0;
		font-size: 13px;
		font-family: ui-monospace, monospace;
	}

	.shell-input {
		width: 180px;
	}

	.text-btn {
		background: none;
		border: none;
		color: #3b82f6;
		font-size: 12px;
		cursor: pointer;
		padding: 4px;
	}

	.text-btn:hover {
		text-decoration: underline;
	}

	.size-controls {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.size-controls input[type="range"] {
		width: 100px;
		accent-color: #22c55e;
	}

	.size-value {
		font-size: 12px;
		color: #94a3b8;
		min-width: 36px;
		font-family: ui-monospace, monospace;
	}

	.settings-footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px;
		border-top: 1px solid #1e1e2e;
		gap: 12px;
	}

	.restart-hint {
		font-size: 11px;
		color: #64748b;
	}

	.footer-buttons {
		display: flex;
		gap: 8px;
	}

	.cancel-btn,
	.save-btn {
		padding: 6px 16px;
		border-radius: 4px;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.15s;
	}

	.cancel-btn {
		background: none;
		border: 1px solid #2d2d44;
		color: #94a3b8;
	}

	.cancel-btn:hover {
		border-color: #3d3d54;
		color: #e2e8f0;
	}

	.save-btn {
		background: #22c55e;
		border: none;
		color: #0a0a0f;
	}

	.save-btn:hover {
		background: #16a34a;
	}
</style>
