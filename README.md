# wiz-term

A standalone terminal emulator with split panes and integrated browser, built with Tauri 2, SvelteKit 2, and xterm.js.

---

## Quick Start

```bash
# Prerequisites: Node.js 18+, Rust 1.70+

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

**Keyboard Shortcuts:**
| Shortcut | Action |
|----------|--------|
| `Cmd+N` | New terminal |
| `Cmd+D` | Split pane (horizontal) |
| `Cmd+Shift+D` | Split pane (vertical) |
| `Cmd+W` | Close focused pane |
| `Cmd+Shift+M` | Toggle minimap |
| `Cmd+[` / `Cmd+]` | Navigate between panes |
| `Cmd++` / `Cmd+-` | Increase/decrease font size |

---

## Overview

wiz-term is extracted from a larger project to serve as a focused, high-performance terminal emulator. It features:

- **Split pane layout** - Horizontal and vertical splits with draggable resizers
- **Integrated browser panes** - Native webviews alongside terminals, sharing cookies like browser tabs
- **WebGL rendering** - Hardware-accelerated terminal rendering via xterm.js
- **Inline images** - Support for imgcat/iTerm2 inline images protocol and SIXEL graphics
- **Minimap window** - Bird's-eye view of all terminal panes with live screenshots
- **SQLite persistence** - Layout and preferences saved locally
- **Native mouse handling** - Smooth scroll, text selection, and context menus work naturally
- **URL detection** - Right-click URLs in terminal to open in browser pane

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Frontend                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  SvelteKit  │  │  xterm.js   │  │  Stores (Svelte 5)  │  │
│  │   Routes    │  │  + WebGL    │  │  - terminal         │  │
│  │             │  │  + Addons   │  │  - terminalBounds   │  │
│  └─────────────┘  └─────────────┘  │  - terminalCanvases │  │
│                                     │  - minimapStore     │  │
│                                     │  - settings         │  │
│                                     └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                     Tauri IPC Bridge                         │
├─────────────────────────────────────────────────────────────┤
│                        Backend (Rust)                        │
│  ┌───────────────────┐  ┌───────────────┐  ┌─────────────┐  │
│  │    PTY Manager    │  │    Webview    │  │   SQLite    │  │
│  │ Sessions/IO/Resize│  │    Manager    │  │  - sessions │  │
│  │                   │  │ Create/Update │  │  - layout   │  │
│  │                   │  │ Navigate/Eval │  │  - prefs    │  │
│  └───────────────────┘  └───────────────┘  └─────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Project Structure

```
wiz-term/
├── package.json                 # Node dependencies
├── svelte.config.js             # SvelteKit with static adapter
├── vite.config.ts               # Vite dev server (port 6292)
├── tailwind.config.js           # Tailwind CSS configuration
├── tsconfig.json
│
├── src/
│   ├── app.css                  # Global styles + terminal theme
│   ├── app.html                 # HTML template
│   │
│   ├── lib/
│   │   ├── api/
│   │   │   └── terminal.ts      # Tauri IPC command wrappers
│   │   │
│   │   ├── components/
│   │   │   ├── shared/
│   │   │   │   └── ResizeHandle.svelte    # Column width resizer
│   │   │   ├── terminal/
│   │   │   │   ├── TerminalLanes.svelte   # Main orchestrator
│   │   │   │   ├── TerminalPane.svelte    # Pane wrapper
│   │   │   │   ├── TerminalLane.svelte    # xterm.js instance
│   │   │   │   ├── SplitContainer.svelte  # Layout tree renderer
│   │   │   │   ├── SplitResizer.svelte    # Split drag handle
│   │   │   │   └── LayoutSlot.svelte      # Bounds measurement
│   │   │   └── webview/
│   │   │       └── WebviewPane.svelte     # Native browser pane
│   │   │
│   │   ├── stores/
│   │   │   ├── terminal.ts          # New terminal events
│   │   │   ├── terminalBounds.ts    # Position/size tracking
│   │   │   ├── terminalCanvases.ts  # Canvas registry for minimap
│   │   │   ├── minimapStore.ts      # Minimap window management
│   │   │   └── settings.ts          # Terminal preferences
│   │   │
│   │   ├── types/
│   │   │   └── terminal.ts          # TypeScript interfaces
│   │   │
│   │   └── utils/
│   │       └── terminalLayout.ts    # Layout tree operations
│   │
│   └── routes/
│       ├── +layout.svelte           # App shell with header
│       ├── +page.svelte             # Empty (layout renders terminals)
│       └── minimap/
│           └── +page.svelte         # Minimap window content
│
├── src-tauri/
│   ├── Cargo.toml                   # Rust dependencies
│   ├── tauri.conf.json              # Tauri configuration
│   ├── build.rs                     # Build script
│   ├── icons/                       # App icons
│   │
│   └── src/
│       ├── main.rs                  # Entry point
│       ├── lib.rs                   # Tauri setup + commands
│       ├── webview.rs               # Native webview management
│       │
│       ├── pty/
│       │   ├── mod.rs               # Module exports
│       │   ├── session.rs           # PTY session management
│       │   └── commands.rs          # Tauri command handlers
│       │
│       └── storage/
│           ├── mod.rs               # Module exports
│           └── database.rs          # SQLite operations
│
└── static/                          # Static assets
```

## Development

### Prerequisites

- **Node.js** 18+ and npm
- **Rust** 1.70+ (install via [rustup](https://rustup.rs))
- **Xcode Command Line Tools** (macOS): `xcode-select --install`

### Setup

```bash
# Clone and enter directory
cd wiz-term

# Install Node dependencies
npm install

# Run in development mode (hot reload for frontend)
npm run tauri dev
```

### External Dependency

The project depends on `xterm-addon-offscreen` for minimap canvas capture:

```json
"xterm-addon-offscreen": "file:../xterm-addon-offscreen"
```

Ensure this package exists as a sibling directory or update the path in `package.json`.

### Available Scripts

| Command | Description |
|---------|-------------|
| `npm run dev` | Start Vite dev server only |
| `npm run build` | Build frontend for production |
| `npm run tauri dev` | Run full Tauri app in dev mode |
| `npm run tauri build` | Build production app bundle |

## Configuration

### Tauri Configuration (`src-tauri/tauri.conf.json`)

Key settings:
- **App identifier**: `app.ljs.wizterm`
- **Dev server**: `http://localhost:6292`
- **Window size**: 1200x800 (min 600x400)

### Terminal Preferences

Preferences are stored in SQLite and can be modified via the settings store:

| Setting | Default | Description |
|---------|---------|-------------|
| `font_size` | 13 | Terminal font size in pixels |
| `font_family` | "JetBrains Mono" | Font family name |
| `scrollback` | 10000 | Lines of scrollback buffer |
| `cursor_blink` | true | Enable cursor blinking |
| `minimap_refresh_ms` | 200 | Minimap update interval |

## Key Components

### TerminalLanes.svelte

The main orchestrator component that:
- Manages the layout tree (splits and terminals)
- Handles session lifecycle (create, close)
- Coordinates keyboard shortcuts
- Sends minimap updates to the minimap window

### TerminalLane.svelte

Individual terminal instance that:
- Initializes xterm.js with WebGL addon
- Handles PTY I/O via Tauri events
- Manages font sizing and search
- Captures canvas for minimap via OffscreenAddon
- Forwards horizontal scroll to parent container for lane navigation
- Supports inline images via ImageAddon (imgcat/SIXEL)
- Right-click context menu for URLs to open in browser pane

### WebviewPane.svelte

Native browser pane that:
- Creates WKWebView child window via Rust backend
- Tracks parent container bounds for positioning
- Provides URL bar with navigation (back/forward/refresh)
- Size presets (S/M/XL) matching terminal lanes
- Shares cookies across all webview instances (like browser tabs)
- Uses Chrome user agent for best site compatibility

### Layout System

The layout is a tree structure supporting both terminals and webviews:

```typescript
type LayoutNode = TerminalNode | WebviewNode | SplitNode;

interface TerminalNode {
  type: 'terminal';
  id: string;
  sessionId: string;
}

interface WebviewNode {
  type: 'webview';
  id: string;
  url: string;
  title?: string;
}

interface SplitNode {
  type: 'split';
  id: string;
  direction: 'horizontal' | 'vertical';
  children: LayoutNode[];
  sizes: number[];  // Percentage sizes
}
```

Operations in `terminalLayout.ts`:
- `addTerminal()` - Add terminal to layout
- `addWebview()` - Add browser pane to layout
- `splitNode()` - Split a node horizontally/vertically
- `removeSession()` - Remove terminal and clean up tree
- `removeNode()` - Remove any node (terminal or webview)
- `findNodeById()` - Locate node in tree
- `getAllWebviews()` - Get all webview nodes

### Minimap System

The minimap provides a bird's-eye view of all terminals:

1. **Canvas Capture**: `TerminalLane` registers canvases with `terminalCanvases` store
2. **Dirty Tracking**: Canvas marked dirty on terminal output
3. **Screenshot**: `TerminalLanes` captures dirty canvases periodically
4. **Window Update**: Screenshots sent to minimap window via Tauri events
5. **Click Navigation**: Clicking minimap thumbnail focuses that terminal

## Database Schema

SQLite database at `~/Library/Application Support/wiz-term/wiz-term.db`:

```sql
-- Terminal session history
CREATE TABLE terminal_sessions (
    id TEXT PRIMARY KEY,
    command TEXT NOT NULL,
    args TEXT NOT NULL,        -- JSON array
    cwd TEXT,
    created_at INTEGER NOT NULL,
    ended_at INTEGER,
    exit_code INTEGER
);

-- Layout persistence
CREATE TABLE terminal_layout (
    id INTEGER PRIMARY KEY DEFAULT 1,
    layout_json TEXT NOT NULL,  -- Serialized layout tree
    updated_at INTEGER NOT NULL
);

-- User preferences
CREATE TABLE terminal_preferences (
    id INTEGER PRIMARY KEY DEFAULT 1,
    font_size INTEGER NOT NULL DEFAULT 13,
    font_family TEXT NOT NULL DEFAULT 'JetBrains Mono',
    scrollback INTEGER NOT NULL DEFAULT 10000,
    cursor_blink INTEGER NOT NULL DEFAULT 1,
    minimap_refresh_ms INTEGER NOT NULL DEFAULT 200,
    updated_at INTEGER NOT NULL
);
```

## Tauri Commands (IPC API)

### Terminal Commands

Commands available from the frontend via `$lib/api/terminal.ts`:

| Command | Description |
|---------|-------------|
| `pty_create_session` | Create new PTY session |
| `pty_write` | Write data to PTY stdin |
| `pty_resize` | Resize PTY dimensions |
| `pty_kill` | Kill PTY session |
| `pty_list_sessions` | List active sessions |
| `pty_get_session` | Get session info |
| `pty_save_layout` | Persist layout to database |
| `pty_get_layout` | Load layout from database |
| `pty_save_preferences` | Save terminal preferences |
| `pty_get_preferences` | Load terminal preferences |

### Webview Commands

Commands for managing native browser panes:

| Command | Description |
|---------|-------------|
| `create_webview` | Create a new child webview at specified position/size |
| `update_webview` | Update webview position and size |
| `close_webview` | Close and remove a webview |
| `navigate_webview` | Navigate webview to a new URL |
| `eval_webview` | Execute JavaScript in webview |

## Building for Production

```bash
# Build optimized app bundle
npm run tauri build

# Output locations:
# macOS: src-tauri/target/release/bundle/macos/WizTerm.app
# DMG:   src-tauri/target/release/bundle/dmg/WizTerm_0.1.0_aarch64.dmg
```

### Build Configuration

The Cargo.toml includes release optimizations:

```toml
[profile.release]
lto = true          # Link-time optimization
opt-level = "z"     # Optimize for size
strip = true        # Strip symbols
```

## Troubleshooting

### Font not rendering correctly

Ensure the font is installed system-wide. Default is JetBrains Mono:
- Download from [JetBrains Mono](https://www.jetbrains.com/lp/mono/)
- Or change font in preferences

### WebGL errors

If terminal rendering fails:
- Check browser/WebView WebGL support
- Try disabling WebGL addon in `TerminalLane.svelte` (falls back to canvas)

### Database locked errors

If you see SQLite lock errors:
- Ensure only one instance of wiz-term is running
- Delete the database to reset: `rm ~/Library/Application\ Support/wiz-term/wiz-term.db`

### Build fails with time crate error

If Rust build fails with time crate version issues:

```bash
cd src-tauri
cargo update time@0.3.47 --precise 0.3.36
```

### zune-jpeg compilation errors

If you see unsafe function errors in zune-jpeg:

```bash
cd src-tauri
cargo update zune-jpeg@0.5.12 --precise 0.5.6
```

### Webview shows degraded/basic HTML

Some sites (like Google) serve simplified pages to webviews. The app uses a Chrome user agent to get full functionality. If sites still look basic:
- Reload the page (right-click → Refresh or click ↻)
- Check that JavaScript is working (try a simple site first)

### Multiple webviews not loading

If opening a second browser pane fails:
- Check the console for errors
- The main window reference must be accessible via `get_window("main")`

## License

MIT

## Credits

- [Tauri](https://tauri.app/) - Desktop app framework
- [SvelteKit](https://kit.svelte.dev/) - Frontend framework
- [xterm.js](https://xtermjs.org/) - Terminal emulator
- [portable-pty](https://docs.rs/portable-pty/) - PTY management
