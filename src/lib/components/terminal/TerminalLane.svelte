<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Terminal } from '@xterm/xterm';
	import { FitAddon } from '@xterm/addon-fit';
	import { WebLinksAddon } from '@xterm/addon-web-links';
	import { SearchAddon } from '@xterm/addon-search';
	import { Unicode11Addon } from '@xterm/addon-unicode11';
	import { WebglAddon } from '@xterm/addon-webgl';
	import { ImageAddon } from '@xterm/addon-image';
	import { OffscreenAddon } from 'xterm-addon-offscreen';
	import '@xterm/xterm/css/xterm.css';
	import { contextMenuStore } from '$lib/stores/contextMenu';
	import {
		writeToSession,
		resizeSession,
		killSession,
		onTerminalOutput,
		onTerminalExit,
		saveImageToTemp
	} from '$lib/api/terminal';
	import type { TerminalSession } from '$lib/types/terminal';
	import { settings } from '$lib/stores/settings';
	import { terminalCanvases } from '$lib/stores/terminalCanvases';
	import { minimapStore } from '$lib/stores/minimapStore';

	interface Props {
		session: TerminalSession;
		nodeId: string;
		visible?: boolean;
		onClose?: () => void;
		onWidthChange?: (width: number) => void;
		onFocus?: (nodeId: string) => void;
		onOpenWebview?: (url: string, title?: string) => void;
	}

	let { session, nodeId, visible = true, onClose, onWidthChange, onFocus, onOpenWebview }: Props = $props();

	const MIN_FONT_SIZE = 8;
	const MAX_FONT_SIZE = 24;

	// Size presets: { width, fontSize }
	const SIZE_PRESETS = {
		s: { width: 320, fontSize: MIN_FONT_SIZE },
		m: { width: 640, fontSize: 13 }, // 80 columns at default font
		xl: { width: 800, fontSize: 13 }
	} as const;

	let containerEl: HTMLDivElement;
	let laneEl: HTMLDivElement;
	let terminal: Terminal | null = null;
	let fitAddon: FitAddon | null = null;
	let unlistenOutput: (() => void) | null = null;
	let unlistenExit: (() => void) | null = null;
	let unlistenDrop: (() => void) | null = null;
	let resizeObserver: ResizeObserver | null = null;
	let intersectionObserver: IntersectionObserver | null = null;
	let isExited = $state(false);

	// Per-lane customization (initially from settings, can be overridden per-lane)
	let fontSize = $state($settings.terminal.font_size);
	let fontFamily = $state($settings.terminal.font_family || 'JetBrains Mono');
	let isFocused = $state(false);
	let customTitle = $state<string | null>(null);

	// Search
	let searchAddon: SearchAddon | null = null;
	let showSearch = $state(false);
	let searchQuery = $state('');
	let searchInputEl: HTMLInputElement;

	// Drag and drop visual feedback
	let isDragOver = $state(false);

	// Offscreen capture addon (for minimap when terminal is off-screen)
	let offscreenAddon: OffscreenAddon | null = null;
	// Cached temp canvas for efficient capture (avoid creating new canvas each frame)
	let tempCaptureCanvas: HTMLCanvasElement | null = null;
	let tempCaptureCtx: CanvasRenderingContext2D | null = null;

	// Get basename of cwd for display
	let cwdBasename = $derived(() => {
		if (!session.cwd) return '~';
		const path = session.cwd.replace(/\/$/, ''); // remove trailing slash
		const parts = path.split('/');
		const name = parts[parts.length - 1];
		// Handle home directory
		if (session.cwd === '~' || session.cwd.endsWith('/~')) return '~';
		return name || '~';
	});

	// Display title: custom title from escape sequence, or fallback to cwd
	let displayTitle = $derived(() => customTitle || cwdBasename());

	onMount(async () => {
		// Read terminal background from CSS variable
		const terminalBg = getComputedStyle(document.documentElement).getPropertyValue('--terminal-bg').trim();

		terminal = new Terminal({
			cursorBlink: $settings.terminal.cursor_blink,
			scrollback: $settings.terminal.scrollback,
			allowProposedApi: true,
			// Critical for Retina/HiDPI display clarity
			devicePixelRatio: window.devicePixelRatio || 1,
			// Font rendering improvements
			fontWeight: '400',
			fontWeightBold: '700',
			letterSpacing: 0,
			minimumContrastRatio: 4.5,
			theme: {
				background: terminalBg,
				foreground: '#e2e8f0',
				cursor: '#22c55e',
				cursorAccent: terminalBg,
				selectionBackground: 'rgba(218, 119, 86, 0.3)',
				black: '#0a0a0f',
				red: '#ef4444',
				green: '#22c55e',
				yellow: '#eab308',
				blue: '#3b82f6',
				magenta: '#a855f7',
				cyan: '#06b6d4',
				white: '#e2e8f0',
				brightBlack: '#64748b',
				brightRed: '#f87171',
				brightGreen: '#4ade80',
				brightYellow: '#facc15',
				brightBlue: '#60a5fa',
				brightMagenta: '#c084fc',
				brightCyan: '#22d3ee',
				brightWhite: '#f8fafc'
			},
			fontFamily: `'${fontFamily}', ui-monospace, 'SF Mono', 'Cascadia Code', 'Fira Code', monospace`,
			fontSize: fontSize,
			lineHeight: 1.2
		});

		fitAddon = new FitAddon();
		terminal.loadAddon(fitAddon);

		// Custom link handler - show context menu on Cmd+Click
		terminal.loadAddon(new WebLinksAddon((event, uri) => {
			console.log('Link clicked:', uri, event);
			event.preventDefault();
			// Show context menu at click position via store
			contextMenuStore.show(event.clientX, event.clientY, [
				{
					label: 'Open in Webview',
					icon: '◧',
					action: () => {
						onOpenWebview?.(uri);
						contextMenuStore.close();
					}
				},
				{
					label: 'Open in Browser',
					icon: '↗',
					action: async () => {
						const { open } = await import('@tauri-apps/plugin-shell');
						await open(uri);
						contextMenuStore.close();
					}
				}
			]);
		}));

		// Search addon
		searchAddon = new SearchAddon();
		terminal.loadAddon(searchAddon);

		// Unicode 11 for better emoji/character width
		const unicode11 = new Unicode11Addon();
		terminal.loadAddon(unicode11);
		terminal.unicode.activeVersion = '11';

		terminal.open(containerEl);

		// WebGL renderer is faster but canvas may look sharper on Retina displays
		// Canvas renderer uses native 2D context with potentially better text antialiasing
		if ($settings.terminal.use_webgl) {
			try {
				terminal.loadAddon(new WebglAddon(true));
			} catch (e) {
				console.warn('WebGL addon failed, using canvas renderer:', e);
			}
		}
		// If use_webgl is false, xterm.js uses its default canvas renderer

		// Load ImageAddon for inline images (SIXEL and iTerm2 IIP protocol)
		// Enables imgcat and similar tools to display images in the terminal
		try {
			const imageAddon = new ImageAddon({
				enableSizeReports: true,
				sixelSupport: true,
				sixelScrolling: true,
				iipSupport: true, // iTerm2 Inline Image Protocol
				iipSizeLimit: 50000000 // 50MB limit for images
			});
			terminal.loadAddon(imageAddon);
		} catch (e) {
			console.warn('ImageAddon failed to load:', e);
		}

		// Load OffscreenAddon for minimap capture (works even when terminal is off-screen)
		// Use higher scale factor for better quality thumbnails
		offscreenAddon = new OffscreenAddon({ scaleFactor: 1.0, showCursor: true });
		terminal.loadAddon(offscreenAddon);

		// Wait for DOM to settle, then fit
		await new Promise((resolve) => setTimeout(resolve, 50));
		fitAddon.fit();

		// Register container for minimap AFTER terminal is fully rendered
		terminalCanvases.register(nodeId, session.id, containerEl);

		// Register capture callback for minimap (uses OffscreenAddon, works off-screen!)
		// Uses renderTo() for optimal performance - renders directly to our temp canvas
		// Accepts optional target dimensions for optimal resolution matching minimap display size
		terminalCanvases.registerCaptureCallback(nodeId, (targetWidth?: number, targetHeight?: number) => {
			if (!offscreenAddon) {
				throw new Error('OffscreenAddon not available');
			}

			// Get full dimensions from addon
			const dims = offscreenAddon.getDimensions();

			// CRITICAL: Validate dimensions to prevent NaN/Infinity from corrupting GPU state
			// This can happen when terminal is newly created but not yet sized
			if (!dims.width || !dims.height || dims.width <= 0 || dims.height <= 0) {
				throw new Error('Terminal not ready for capture');
			}

			// Calculate capture dimensions: use target if provided, otherwise full resolution
			let captureWidth = dims.width;
			let captureHeight = dims.height;

			if (targetWidth && targetHeight && targetWidth > 0 && targetHeight > 0) {
				// Scale to fit target dimensions while preserving aspect ratio
				const sourceAspect = dims.width / dims.height;
				const targetAspect = targetWidth / targetHeight;

				if (sourceAspect > targetAspect) {
					// Source is wider - fit to target width
					captureWidth = targetWidth;
					captureHeight = Math.round(targetWidth / sourceAspect);
				} else {
					// Source is taller - fit to target height
					captureHeight = targetHeight;
					captureWidth = Math.round(targetHeight * sourceAspect);
				}

				// Ensure minimum dimensions for quality
				captureWidth = Math.max(captureWidth, 100);
				captureHeight = Math.max(captureHeight, 50);
			}

			// Final safety check: ensure no NaN/Infinity/zero dimensions
			if (!Number.isFinite(captureWidth) || !Number.isFinite(captureHeight) ||
			    captureWidth <= 0 || captureHeight <= 0) {
				throw new Error('Invalid capture dimensions');
			}

			// Create/resize cached temp canvas as needed
			if (!tempCaptureCanvas || !tempCaptureCtx) {
				tempCaptureCanvas = document.createElement('canvas');
				tempCaptureCtx = tempCaptureCanvas.getContext('2d')!;
			}
			if (tempCaptureCanvas.width !== captureWidth || tempCaptureCanvas.height !== captureHeight) {
				tempCaptureCanvas.width = captureWidth;
				tempCaptureCanvas.height = captureHeight;
			}

			// Render with scaling if needed
			offscreenAddon.renderTo(tempCaptureCtx, { width: captureWidth, height: captureHeight });
			return tempCaptureCanvas.toDataURL(); // PNG - faster encoding than JPEG for terminal content
		});

		// Send initial size to backend - use resize dance to ensure SIGWINCH is triggered
		// This fixes ncurses apps (htop, vim) that only send incremental updates after reconnect
		if (terminal.cols > 1 && terminal.rows > 1) {
			await resizeSession(session.id, terminal.cols - 1, terminal.rows);
			await resizeSession(session.id, terminal.cols, terminal.rows);
		} else {
			await resizeSession(session.id, terminal.cols, terminal.rows);
		}

		// Handle user input
		terminal.onData(async (data) => {
			if (!isExited) {
				const encoder = new TextEncoder();
				await writeToSession(session.id, encoder.encode(data));
			}
		});

		// Listen for title changes (OSC escape sequences)
		terminal.onTitleChange((title) => {
			customTitle = title || null;
		});

    // Listen for output from this session
		unlistenOutput = await onTerminalOutput((output) => {
			if (output.session_id === session.id && terminal) {
				terminal.write(new Uint8Array(output.data));
				// Force refresh even when not visible (for minimap capture)
				// WebGL canvas won't update if element is out of viewport otherwise
				terminal.refresh(0, terminal.rows - 1);
				// Mark dirty and schedule minimap update (event-driven, not polling)
				terminalCanvases.markDirty(nodeId);
				minimapStore.scheduleUpdate();
			}
		});

		// Listen for exit
		unlistenExit = await onTerminalExit((exit) => {
			if (exit.session_id === session.id && terminal) {
				isExited = true;
				terminal.write('\r\n\x1b[90m[Process exited');
				if (exit.exit_code !== null) {
					terminal.write(` with code ${exit.exit_code}`);
				}
				terminal.write(']\x1b[0m\r\n');
			}
		});

		// Handle resize
		resizeObserver = new ResizeObserver(() => {
			if (fitAddon && terminal) {
				fitAddon.fit();
				// Force WebGL canvas to re-render after resize
				requestAnimationFrame(() => {
					terminal?.refresh(0, terminal.rows - 1);
				});
				resizeSession(session.id, terminal.cols, terminal.rows);
				// Update dimensions for serialize-based capture
				terminalCanvases.updateDimensions(nodeId, terminal.cols, terminal.rows);
				// Mark dirty and schedule minimap update
				terminalCanvases.markDirty(nodeId);
				minimapStore.scheduleUpdate();
			}
		});
		resizeObserver.observe(containerEl);

		// Listen for Tauri file drop events - gives us real file paths
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		unlistenDrop = await getCurrentWindow().onDragDropEvent(async (event) => {
			if (event.payload.type === 'drop' && isFocused) {
				const paths = event.payload.paths;
				if (paths && paths.length > 0) {
					// Inject file paths directly - no conversion needed
					// Claude Code (or any CLI) can read the files directly
					const encoder = new TextEncoder();
					for (const filePath of paths) {
						await writeToSession(session.id, encoder.encode(filePath + ' '));
					}
					isDragOver = false;
				}
			} else if (event.payload.type === 'over') {
				// Could track hover state here if needed
			} else if (event.payload.type === 'leave') {
				isDragOver = false;
			}
		});

		// Handle visibility changes (when switching views)
		intersectionObserver = new IntersectionObserver(
			(entries) => {
				for (const entry of entries) {
					if (entry.isIntersecting && terminal && fitAddon) {
						// Re-fit and refresh when becoming visible
						requestAnimationFrame(() => {
							fitAddon?.fit();
							terminal?.refresh(0, terminal.rows - 1);
						});
					}
				}
			},
			{ threshold: 0.1 }
		);
		intersectionObserver.observe(containerEl);
	});

	onDestroy(() => {
		unlistenOutput?.();
		unlistenExit?.();
		unlistenDrop?.();
		resizeObserver?.disconnect();
		intersectionObserver?.disconnect();
		terminalCanvases.unregisterRefreshCallback(nodeId);
		terminalCanvases.unregisterSerializeCallback(nodeId);
		terminalCanvases.unregisterCaptureCallback(nodeId);
		terminalCanvases.unregister(nodeId);
		offscreenAddon?.dispose();
		terminal?.dispose();
		// Clean up cached capture canvas
		tempCaptureCanvas = null;
		tempCaptureCtx = null;
	});

	// Refresh terminal when visibility changes (e.g., navigating back to terminal page)
	$effect(() => {
		if (visible && terminal && fitAddon) {
			// Small delay to ensure DOM has updated
			requestAnimationFrame(() => {
				fitAddon?.fit();
				terminal?.refresh(0, terminal.rows - 1);
				resizeSession(session.id, terminal!.cols, terminal!.rows);
			});
		}
	});

	async function handleClose() {
		if (!isExited) {
			try {
				await killSession(session.id);
			} catch (e) {
				// Session may already be dead on backend - still close the UI
				console.warn('Failed to kill session:', e);
			}
		}
		onClose?.();
	}

	// Font size adjustment
	function changeFontSize(delta: number) {
		const newSize = Math.max(MIN_FONT_SIZE, Math.min(MAX_FONT_SIZE, fontSize + delta));
		if (newSize !== fontSize && terminal) {
			fontSize = newSize;
			terminal.options.fontSize = newSize;
			fitAddon?.fit();
			resizeSession(session.id, terminal.cols, terminal.rows);
		}
	}

	// Apply size preset (S/M/XL)
	function applyPreset(preset: keyof typeof SIZE_PRESETS) {
		const { width, fontSize: newFontSize } = SIZE_PRESETS[preset];

		// Update font size
		if (newFontSize !== fontSize && terminal) {
			fontSize = newFontSize;
			terminal.options.fontSize = newFontSize;
			fitAddon?.fit();
			resizeSession(session.id, terminal.cols, terminal.rows);
		}

		// Update column width
		onWidthChange?.(width);
	}

	function handleKeydown(e: KeyboardEvent) {
		// Only handle if this lane is focused
		if (!isFocused) return;

		// Cmd/Ctrl + F to open search
		if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
			e.preventDefault();
			showSearch = true;
			// Focus input after it renders
			requestAnimationFrame(() => searchInputEl?.focus());
			return;
		}

		// Escape to close search
		if (e.key === 'Escape' && showSearch) {
			e.preventDefault();
			closeSearch();
			return;
		}

		// Cmd/Ctrl + Plus to increase font size
		if ((e.metaKey || e.ctrlKey) && (e.key === '=' || e.key === '+')) {
			e.preventDefault();
			changeFontSize(1);
		}
		// Cmd/Ctrl + Minus to decrease font size
		if ((e.metaKey || e.ctrlKey) && e.key === '-') {
			e.preventDefault();
			changeFontSize(-1);
		}
		// Cmd/Ctrl + 0 to reset font size to settings default
		if ((e.metaKey || e.ctrlKey) && e.key === '0') {
			e.preventDefault();
			const defaultSize = $settings.terminal.font_size;
			if (fontSize !== defaultSize && terminal) {
				fontSize = defaultSize;
				terminal.options.fontSize = defaultSize;
				fitAddon?.fit();
				resizeSession(session.id, terminal.cols, terminal.rows);
			}
		}
	}

	// Search functions
	function closeSearch() {
		showSearch = false;
		searchQuery = '';
		searchAddon?.clearDecorations();
	}

	function handleSearchKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			if (e.shiftKey) {
				searchAddon?.findPrevious(searchQuery);
			} else {
				searchAddon?.findNext(searchQuery);
			}
		}
		if (e.key === 'Escape') {
			e.preventDefault();
			closeSearch();
		}
	}

	function handleSearchInput() {
		if (searchQuery) {
			searchAddon?.findNext(searchQuery);
		} else {
			searchAddon?.clearDecorations();
		}
	}

	function handleFocus() {
		isFocused = true;
		onFocus?.(nodeId);
	}

	function handleBlur(e: FocusEvent) {
		// Only blur if focus is leaving the lane entirely
		if (!laneEl?.contains(e.relatedTarget as Node)) {
			isFocused = false;
		}
	}

	// Handle wheel events: vertical scroll goes to xterm (scrollback), horizontal scroll bubbles up
	function handleWheel(e: WheelEvent) {
		// Determine scroll direction - check if primarily horizontal
		const absX = Math.abs(e.deltaX);
		const absY = Math.abs(e.deltaY);

		// If horizontal scroll is dominant (or significant), forward it to the lanes container
		// Use a threshold to handle trackpad diagonal scrolls
		if (absX > 0 && absX >= absY * 0.5) {
			// Find the lanes container (parent with overflow-x: auto)
			const lanesContainer = containerEl?.closest('.lanes-container');
			if (lanesContainer) {
				// Forward horizontal scroll to lanes container
				lanesContainer.scrollLeft += e.deltaX;

				// If this was primarily horizontal, prevent xterm from handling it
				if (absX > absY) {
					e.preventDefault();
					e.stopPropagation();
				}
			}
		}
		// Vertical scroll passes through to xterm naturally
	}

	// Handle paste events - check for images in clipboard
	async function handlePaste(e: ClipboardEvent) {
		const items = e.clipboardData?.items;
		if (!items) return;

		for (const item of items) {
			if (item.type.startsWith('image/')) {
				e.preventDefault();
				const file = item.getAsFile();
				if (file) {
					await injectImageAsPath(file);
				}
				return;
			}
		}
		// Non-image paste handled by xterm naturally
	}

	// Handle drop events - we suppress the HTML5 drop since Tauri's onDragDropEvent handles it
	function handleDrop(e: DragEvent) {
		e.preventDefault();
		e.stopPropagation();
		isDragOver = false;
		// Actual file handling is done by Tauri's onDragDropEvent in onMount
	}

	function handleDragOver(e: DragEvent) {
		// Accept file drops
		if (e.dataTransfer?.types.includes('Files')) {
			e.preventDefault();
			e.stopPropagation();
			isDragOver = true;
			if (e.dataTransfer) {
				e.dataTransfer.dropEffect = 'copy';
			}
		}
	}

	function handleDragLeave(e: DragEvent) {
		e.preventDefault();
		isDragOver = false;
	}

	// Convert image file to base64 data URL and inject path into PTY
	async function injectImageAsPath(file: File) {
		if (isExited) return;

		try {
			// Read file as base64 data URL
			const dataUrl = await new Promise<string>((resolve, reject) => {
				const reader = new FileReader();
				reader.onload = () => resolve(reader.result as string);
				reader.onerror = () => reject(new Error('Failed to read image file'));
				reader.readAsDataURL(file);
			});

			// Save to temp file via Tauri backend
			const tempPath = await saveImageToTemp(dataUrl, file.name);

			// Inject the file path into the terminal
			// This is what Claude Code expects - a file path it can read
			const encoder = new TextEncoder();
			await writeToSession(session.id, encoder.encode(tempPath));
		} catch (err) {
			console.error('Failed to process image:', err);
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="terminal-lane"
	class:focused={isFocused}
	bind:this={laneEl}
	onfocusin={handleFocus}
	onfocusout={handleBlur}
>
	<div class="lane-header">
		<span class="status-dot" class:alive={session.is_alive && !isExited} class:dead={isExited} title={isExited ? 'Exited' : 'Running'}></span>
		<span class="session-info" title={customTitle ? `${customTitle} (~/​${cwdBasename()})` : `~/${cwdBasename()}`}>
			{#if customTitle}
				{customTitle}
			{:else}
				<span class="prefix">~/</span>{cwdBasename()}
			{/if}
		</span>
		<div class="size-presets">
			<button class="size-btn" onclick={() => applyPreset('s')} title="Small (120px, tiny font)">s</button>
			<button class="size-btn" onclick={() => applyPreset('m')} title="Medium (480px)">m</button>
			<button class="size-btn" onclick={() => applyPreset('xl')} title="Extra Large (800px)">xl</button>
		</div>
		<button class="close-btn" onclick={handleClose} title="Close terminal">×</button>
	</div>

	{#if showSearch}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="search-bar" onkeydown={handleSearchKeydown}>
			<input
				type="text"
				bind:this={searchInputEl}
				bind:value={searchQuery}
				placeholder="Search..."
				oninput={handleSearchInput}
			/>
			<button onclick={() => searchAddon?.findPrevious(searchQuery)} title="Previous (Shift+Enter)">↑</button>
			<button onclick={() => searchAddon?.findNext(searchQuery)} title="Next (Enter)">↓</button>
			<button onclick={closeSearch} title="Close (Esc)">×</button>
		</div>
	{/if}

	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="terminal-container"
		class:drag-over={isDragOver}
		bind:this={containerEl}
		oncontextmenu={(e) => e.preventDefault()}
		onwheel={handleWheel}
		onpaste={handlePaste}
		ondrop={handleDrop}
		ondragover={handleDragOver}
		ondragleave={handleDragLeave}
		ondragenter={(e) => { e.preventDefault(); e.stopPropagation(); }}
	></div>
</div>

<style>
	.terminal-lane {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		min-height: 0;
		min-width: 0;
		background: var(--terminal-bg);
		user-select: none;
		overflow: hidden;
		overscroll-behavior: none;
	}


	.lane-header {
		padding: 4px 8px;
		background: #0f0f1a;
		border-bottom: 1px solid #1e1e2e;
		display: flex;
		align-items: center;
		flex-shrink: 0;
		gap: 6px;
	}

	.status-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.status-dot.alive {
		background: #22c55e;
	}

	.status-dot.dead {
		background: #64748b;
	}

	.session-info {
		font-size: 13px;
		color: #94a3b8;
		font-family: ui-monospace, 'SF Mono', 'Cascadia Code', 'Fira Code', monospace;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex: 1;
		min-width: 0;
	}

	.session-info .prefix {
		color: #22c55e;
	}

	.size-presets {
		display: flex;
		gap: 2px;
		margin-left: auto;
	}

	.size-btn {
		background: none;
		border: 1px solid transparent;
		color: #4b5563;
		cursor: pointer;
		font-size: 11px;
		font-weight: 500;
		line-height: 1;
		padding: 2px 4px;
		border-radius: 2px;
		transition: all 0.15s;
		font-family: ui-monospace, 'SF Mono', monospace;
		text-transform: uppercase;
	}

	.size-btn:hover {
		color: #94a3b8;
		border-color: #374151;
		background: rgba(255, 255, 255, 0.05);
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

	.search-bar {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 4px 8px;
		background: #0f0f1a;
		border-bottom: 1px solid #1e1e2e;
	}

	.search-bar input {
		flex: 1;
		background: #1a1a2e;
		border: 1px solid #2d2d44;
		border-radius: 3px;
		padding: 4px 8px;
		color: #e2e8f0;
		font-size: 11px;
		font-family: ui-monospace, 'SF Mono', 'Cascadia Code', 'Fira Code', monospace;
		outline: none;
		min-width: 0;
	}

	.search-bar input:focus {
		border-color: #3b82f6;
	}

	.search-bar input::placeholder {
		color: #64748b;
	}

	.search-bar button {
		background: none;
		border: none;
		color: #64748b;
		cursor: pointer;
		padding: 2px 6px;
		font-size: 12px;
		border-radius: 3px;
		transition: all 0.15s;
	}

	.search-bar button:hover {
		color: #e2e8f0;
		background: rgba(255, 255, 255, 0.1);
	}

	.terminal-container {
		flex: 1;
		min-height: 0;
		padding: 8px;
		/* Ensure padding area matches terminal background */
		background: var(--terminal-bg);
		overflow: hidden;
		overscroll-behavior: none;
		/* Force GPU layer to keep WebGL rendering even when scrolled off-screen */
		transform: translateZ(0);
		content-visibility: visible;
		/* Font smoothing for clearer text on macOS */
		-webkit-font-smoothing: antialiased;
		-moz-osx-font-smoothing: grayscale;
		/* Drag and drop visual feedback */
		transition: box-shadow 0.15s ease;
	}

	.terminal-container.drag-over {
		box-shadow: inset 0 0 0 2px #3b82f6;
	}

	.terminal-container :global(.xterm) {
		height: 100%;
	}

	/* Ensure WebGL canvas stays rendered */
	.terminal-container :global(canvas) {
		content-visibility: visible;
	}

	.terminal-container :global(.xterm-viewport) {
		overflow-y: auto !important;
		/* Disable horizontal scroll in xterm - let it bubble up to lanes container */
		overflow-x: hidden !important;
		/* Contain vertical scroll, prevent any overscroll effects */
		overscroll-behavior: contain;
	}
</style>
