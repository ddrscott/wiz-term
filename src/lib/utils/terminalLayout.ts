import type {
	LayoutNode,
	TerminalNode,
	WebviewNode,
	SplitNode,
	TerminalLayout,
	SplitDirection,
	DropZone
} from '$lib/types/terminal';

// Generate unique IDs
export function generateId(): string {
	return crypto.randomUUID().slice(0, 8);
}

// Create a new terminal node
export function createTerminalNode(sessionId: string): TerminalNode {
	return {
		type: 'terminal',
		id: generateId(),
		sessionId
	};
}

// Create a new webview node
export function createWebviewNode(url: string, title?: string): WebviewNode {
	return {
		type: 'webview',
		id: generateId(),
		url,
		title
	};
}

// Create empty layout
export function createEmptyLayout(): TerminalLayout {
	return {
		root: null,
		version: 1
	};
}

// Create layout with a single terminal
export function createLayoutWithTerminal(sessionId: string): TerminalLayout {
	return {
		root: createTerminalNode(sessionId),
		version: 1
	};
}

// Deep clone a layout node
function cloneNode(node: LayoutNode): LayoutNode {
	if (node.type === 'terminal' || node.type === 'webview') {
		return { ...node };
	}
	return {
		...node,
		children: node.children.map(cloneNode),
		sizes: [...node.sizes]
	};
}

// Deep clone a layout
export function cloneLayout(layout: TerminalLayout): TerminalLayout {
	return {
		root: layout.root ? cloneNode(layout.root) : null,
		version: layout.version
	};
}

// Find a node by its ID
export function findNodeById(layout: TerminalLayout, nodeId: string): LayoutNode | null {
	if (!layout.root) return null;

	function search(node: LayoutNode): LayoutNode | null {
		if (node.id === nodeId) return node;
		if (node.type === 'split') {
			for (const child of node.children) {
				const found = search(child);
				if (found) return found;
			}
		}
		return null;
	}

	return search(layout.root);
}

// Find a node by session ID
export function findNodeBySessionId(
	layout: TerminalLayout,
	sessionId: string
): TerminalNode | null {
	if (!layout.root) return null;

	function search(node: LayoutNode): TerminalNode | null {
		if (node.type === 'terminal' && node.sessionId === sessionId) {
			return node;
		}
		if (node.type === 'split') {
			for (const child of node.children) {
				const found = search(child);
				if (found) return found;
			}
		}
		return null;
	}

	return search(layout.root);
}

// Find parent of a node
export function findParent(
	layout: TerminalLayout,
	nodeId: string
): { parent: SplitNode; index: number } | null {
	if (!layout.root) return null;

	function search(node: LayoutNode): { parent: SplitNode; index: number } | null {
		if (node.type === 'split') {
			for (let i = 0; i < node.children.length; i++) {
				if (node.children[i].id === nodeId) {
					return { parent: node, index: i };
				}
				const found = search(node.children[i]);
				if (found) return found;
			}
		}
		return null;
	}

	return search(layout.root);
}

// Get all session IDs in the layout (terminal nodes only)
export function getAllSessionIds(layout: TerminalLayout): string[] {
	const sessionIds: string[] = [];

	function collect(node: LayoutNode) {
		if (node.type === 'terminal') {
			sessionIds.push(node.sessionId);
		} else if (node.type === 'split') {
			node.children.forEach(collect);
		}
	}

	if (layout.root) {
		collect(layout.root);
	}

	return sessionIds;
}

// Get all webview nodes in the layout
export function getAllWebviews(layout: TerminalLayout): WebviewNode[] {
	const webviews: WebviewNode[] = [];

	function collect(node: LayoutNode) {
		if (node.type === 'webview') {
			webviews.push(node);
		} else if (node.type === 'split') {
			node.children.forEach(collect);
		}
	}

	if (layout.root) {
		collect(layout.root);
	}

	return webviews;
}

// Find webview by URL
export function findWebviewByUrl(layout: TerminalLayout, url: string): WebviewNode | null {
	if (!layout.root) return null;

	function search(node: LayoutNode): WebviewNode | null {
		if (node.type === 'webview' && node.url === url) {
			return node;
		}
		if (node.type === 'split') {
			for (const child of node.children) {
				const found = search(child);
				if (found) return found;
			}
		}
		return null;
	}

	return search(layout.root);
}

// Update webview URL and/or title
export function updateWebview(
	layout: TerminalLayout,
	nodeId: string,
	updates: { url?: string; title?: string }
): TerminalLayout {
	if (!layout.root) return layout;

	const newLayout = cloneLayout(layout);

	function update(node: LayoutNode): LayoutNode {
		if (node.type === 'webview' && node.id === nodeId) {
			return {
				...node,
				url: updates.url ?? node.url,
				title: updates.title ?? node.title
			};
		}
		if (node.type === 'split') {
			return {
				...node,
				children: node.children.map(update)
			};
		}
		return node;
	}

	return {
		root: update(newLayout.root!) as LayoutNode,
		version: layout.version
	};
}

// Convert drop zone to split direction
export function dropZoneToDirection(zone: DropZone): SplitDirection {
	return zone === 'left' || zone === 'right' ? 'horizontal' : 'vertical';
}

// Check if new node should come first based on drop zone
export function isNewNodeFirst(zone: DropZone): boolean {
	return zone === 'left' || zone === 'top';
}

// Add a terminal to the layout at the root level (flat horizontal layout)
export function addTerminal(layout: TerminalLayout, sessionId: string): TerminalLayout {
	const newNode = createTerminalNode(sessionId);

	if (!layout.root) {
		return {
			root: newNode,
			version: layout.version
		};
	}

	// If root is already a horizontal split, add to it
	if (layout.root.type === 'split' && layout.root.direction === 'horizontal') {
		const existingRoot = layout.root;
		const numChildren = existingRoot.children.length;
		const newSize = 100 / (numChildren + 1);
		const scaleFactor = numChildren / (numChildren + 1);

		return {
			root: {
				...existingRoot,
				children: [newNode, ...existingRoot.children.map(cloneNode)],
				sizes: [newSize, ...existingRoot.sizes.map((s) => s * scaleFactor)]
			},
			version: layout.version
		};
	}

	// Otherwise, create a new horizontal split at root
	const newRoot: SplitNode = {
		type: 'split',
		id: generateId(),
		direction: 'horizontal',
		children: [newNode, cloneNode(layout.root)],
		sizes: [50, 50]
	};

	return {
		root: newRoot,
		version: layout.version
	};
}

// Add a webview to the right of a target node (or at root level if no target)
export function addWebview(
	layout: TerminalLayout,
	url: string,
	title?: string,
	targetNodeId?: string
): TerminalLayout {
	const newNode = createWebviewNode(url, title);

	if (!layout.root) {
		return {
			root: newNode,
			version: layout.version
		};
	}

	// If target specified, insert after that column
	if (targetNodeId) {
		return insertNodeAfter(layout, targetNodeId, newNode);
	}

	// Otherwise add to root level
	if (layout.root.type === 'split' && layout.root.direction === 'horizontal') {
		const existingRoot = layout.root;
		const numChildren = existingRoot.children.length;
		const newSize = 100 / (numChildren + 1);
		const scaleFactor = numChildren / (numChildren + 1);

		return {
			root: {
				...existingRoot,
				children: [...existingRoot.children.map(cloneNode), newNode],
				sizes: [...existingRoot.sizes.map((s) => s * scaleFactor), newSize]
			},
			version: layout.version
		};
	}

	// Create horizontal split
	const newRoot: SplitNode = {
		type: 'split',
		id: generateId(),
		direction: 'horizontal',
		children: [cloneNode(layout.root), newNode],
		sizes: [50, 50]
	};

	return {
		root: newRoot,
		version: layout.version
	};
}

// Generic function to insert a node after another node's column
function insertNodeAfter(
	layout: TerminalLayout,
	targetNodeId: string,
	newNode: LayoutNode
): TerminalLayout {
	if (!layout.root) {
		return { root: newNode, version: layout.version };
	}

	// If root is a single node (terminal or webview)
	if (layout.root.type === 'terminal' || layout.root.type === 'webview') {
		if (layout.root.id === targetNodeId) {
			const newRoot: SplitNode = {
				type: 'split',
				id: generateId(),
				direction: 'horizontal',
				children: [cloneNode(layout.root), newNode],
				sizes: [50, 50]
			};
			return { root: newRoot, version: layout.version };
		}
		return layout;
	}

	// Root is a split - find which column contains the target
	const rootSplit = layout.root; // Type narrowed to SplitNode
	if (rootSplit.direction === 'horizontal') {
		let targetColumnIndex = -1;
		for (let i = 0; i < rootSplit.children.length; i++) {
			const child = rootSplit.children[i];
			if (child.id === targetNodeId) {
				targetColumnIndex = i;
				break;
			}
			if (child.type === 'split') {
				function containsNode(node: LayoutNode): boolean {
					if (node.id === targetNodeId) return true;
					if (node.type === 'split') {
						return node.children.some(containsNode);
					}
					return false;
				}
				if (containsNode(child)) {
					targetColumnIndex = i;
					break;
				}
			}
		}

		if (targetColumnIndex !== -1) {
			const newChildren = [...rootSplit.children];
			newChildren.splice(targetColumnIndex + 1, 0, newNode);
			const numChildren = newChildren.length;
			const equalSize = 100 / numChildren;

			return {
				root: {
					...rootSplit,
					children: newChildren.map(cloneNode),
					sizes: newChildren.map(() => equalSize)
				},
				version: layout.version
			};
		}
	}

	// Fallback: wrap in horizontal split
	const newRoot: SplitNode = {
		type: 'split',
		id: generateId(),
		direction: 'horizontal',
		children: [cloneNode(layout.root), newNode],
		sizes: [50, 50]
	};
	return { root: newRoot, version: layout.version };
}

// Insert a new terminal column after the column containing the target node
// Used for Cmd+D horizontal splits to maintain independent column widths
export function insertTerminalAfter(
	layout: TerminalLayout,
	targetNodeId: string,
	newSessionId: string
): TerminalLayout {
	if (!layout.root) {
		return createLayoutWithTerminal(newSessionId);
	}

	const newNode = createTerminalNode(newSessionId);

	// If root is a single terminal
	if (layout.root.type === 'terminal') {
		if (layout.root.id === targetNodeId) {
			// Create horizontal split with new terminal after the original
			const newRoot: SplitNode = {
				type: 'split',
				id: generateId(),
				direction: 'horizontal',
				children: [cloneNode(layout.root), newNode],
				sizes: [50, 50]
			};
			return { root: newRoot, version: layout.version };
		}
		return layout;
	}

	// Root must be a split at this point (terminal case handled above)
	if (layout.root.type !== 'split') {
		// Fallback: wrap in horizontal split
		const newRoot: SplitNode = {
			type: 'split',
			id: generateId(),
			direction: 'horizontal',
			children: [cloneNode(layout.root), newNode],
			sizes: [50, 50]
		};
		return { root: newRoot, version: layout.version };
	}

	const rootSplit = layout.root;

	// Root is a split - find which column contains the target
	if (rootSplit.direction === 'horizontal') {
		// Find the index of the column containing the target
		let targetColumnIndex = -1;
		for (let i = 0; i < rootSplit.children.length; i++) {
			const child = rootSplit.children[i];
			if (child.id === targetNodeId) {
				targetColumnIndex = i;
				break;
			}
			if (child.type === 'split') {
				// Check if target is nested inside this column
				function containsNode(node: LayoutNode): boolean {
					if (node.id === targetNodeId) return true;
					if (node.type === 'split') {
						return node.children.some(containsNode);
					}
					return false;
				}
				if (containsNode(child)) {
					targetColumnIndex = i;
					break;
				}
			}
		}

		if (targetColumnIndex !== -1) {
			// Insert new column after the target column
			const newChildren = [...rootSplit.children];

			// Insert new node after target column
			newChildren.splice(targetColumnIndex + 1, 0, newNode);

			// Recalculate sizes - give new column equal share
			const numChildren = newChildren.length;
			const equalSize = 100 / numChildren;
			const newSizesArray = newChildren.map(() => equalSize);

			return {
				root: {
					...rootSplit,
					children: newChildren.map(cloneNode),
					sizes: newSizesArray
				},
				version: layout.version
			};
		}
	}

	// Root is vertical split or target not found - wrap in horizontal split
	const newRoot: SplitNode = {
		type: 'split',
		id: generateId(),
		direction: 'horizontal',
		children: [cloneNode(layout.root), newNode],
		sizes: [50, 50]
	};
	return { root: newRoot, version: layout.version };
}

// Split a node (creates new split containing the target and new terminal)
export function splitNode(
	layout: TerminalLayout,
	targetNodeId: string,
	zone: DropZone,
	newSessionId: string
): TerminalLayout {
	if (!layout.root) {
		return createLayoutWithTerminal(newSessionId);
	}

	const newLayout = cloneLayout(layout);
	const direction = dropZoneToDirection(zone);
	const newFirst = isNewNodeFirst(zone);
	const newTerminal = createTerminalNode(newSessionId);

	// Helper to replace a node with a split containing it and the new terminal
	function replaceWithSplit(node: LayoutNode): LayoutNode {
		if (node.id === targetNodeId) {
			const children = newFirst ? [newTerminal, cloneNode(node)] : [cloneNode(node), newTerminal];
			return {
				type: 'split',
				id: generateId(),
				direction,
				children,
				sizes: [50, 50]
			};
		}
		if (node.type === 'split') {
			return {
				...node,
				children: node.children.map(replaceWithSplit)
			};
		}
		return node;
	}

	return {
		root: replaceWithSplit(newLayout.root!) as LayoutNode,
		version: layout.version
	};
}

// Remove a node from the layout
export function removeNode(layout: TerminalLayout, nodeId: string): TerminalLayout {
	if (!layout.root) return layout;

	// If removing the root terminal, return empty layout
	if (layout.root.id === nodeId) {
		return createEmptyLayout();
	}

	const newLayout = cloneLayout(layout);

	function remove(node: LayoutNode): LayoutNode | null {
		// Terminal and webview nodes are leaf nodes - return as-is
		if (node.type === 'terminal' || node.type === 'webview') {
			return node;
		}

		// node is a split - filter out the node to remove
		const newChildren: LayoutNode[] = [];
		const newSizes: number[] = [];
		let removedIndex = -1;

		for (let i = 0; i < node.children.length; i++) {
			if (node.children[i].id === nodeId) {
				removedIndex = i;
			} else {
				const processedChild = remove(node.children[i]);
				if (processedChild) {
					newChildren.push(processedChild);
					newSizes.push(node.sizes[i]);
				}
			}
		}

		// If we removed a child and there's only one left, return that child (collapse split)
		if (removedIndex !== -1 && newChildren.length === 1) {
			return newChildren[0];
		}

		// If nothing was removed at this level but splits might have collapsed below
		if (removedIndex === -1 && newChildren.length < node.children.length) {
			if (newChildren.length === 1) {
				return newChildren[0];
			}
		}

		// Normalize sizes to sum to 100
		if (newSizes.length > 0) {
			const total = newSizes.reduce((a, b) => a + b, 0);
			const normalizedSizes = newSizes.map((s) => (s / total) * 100);
			return {
				...node,
				children: newChildren,
				sizes: normalizedSizes
			};
		}

		return null;
	}

	const newRoot = remove(newLayout.root!);
	return {
		root: newRoot,
		version: layout.version
	};
}

// Remove a session from the layout
export function removeSession(layout: TerminalLayout, sessionId: string): TerminalLayout {
	const node = findNodeBySessionId(layout, sessionId);
	if (!node) return layout;
	return removeNode(layout, node.id);
}

// Update sizes for a split node
export function resizeSplit(
	layout: TerminalLayout,
	splitNodeId: string,
	sizes: number[]
): TerminalLayout {
	if (!layout.root) return layout;

	const newLayout = cloneLayout(layout);

	function update(node: LayoutNode): LayoutNode {
		if (node.id === splitNodeId && node.type === 'split') {
			return {
				...node,
				sizes: [...sizes]
			};
		}
		if (node.type === 'split') {
			return {
				...node,
				children: node.children.map(update)
			};
		}
		return node;
	}

	return {
		root: update(newLayout.root!) as LayoutNode,
		version: layout.version
	};
}

// Move a session to a different location (used for center drop zone)
export function moveSession(
	layout: TerminalLayout,
	sessionId: string,
	targetNodeId: string
): TerminalLayout {
	// For now, moving to center just swaps positions
	// This could be enhanced to support tab-like behavior later
	const sourceNode = findNodeBySessionId(layout, sessionId);
	const targetNode = findNodeById(layout, targetNodeId);

	if (!sourceNode || !targetNode || targetNode.type !== 'terminal') {
		return layout;
	}

	if (sourceNode.id === targetNode.id) {
		return layout; // Same node, no change
	}

	// Capture as non-null for closure
	const srcNode = sourceNode;
	const tgtNode = targetNode;
	const newLayout = cloneLayout(layout);

	// Swap session IDs
	function swap(node: LayoutNode): LayoutNode {
		if (node.type === 'terminal') {
			if (node.id === srcNode.id) {
				return { ...node, sessionId: tgtNode.sessionId };
			}
			if (node.id === tgtNode.id) {
				return { ...node, sessionId: srcNode.sessionId };
			}
		}
		if (node.type === 'split') {
			return {
				...node,
				children: node.children.map(swap)
			};
		}
		return node;
	}

	return {
		root: newLayout.root ? swap(newLayout.root) : null,
		version: layout.version
	};
}

// Serialize layout to JSON string
export function serializeLayout(layout: TerminalLayout): string {
	return JSON.stringify(layout);
}

// Deserialize layout from JSON string
export function deserializeLayout(json: string): TerminalLayout | null {
	try {
		const parsed = JSON.parse(json);
		// Basic validation
		if (typeof parsed.version !== 'number') {
			return null;
		}
		return parsed as TerminalLayout;
	} catch {
		return null;
	}
}

// Count terminals in layout
export function countTerminals(layout: TerminalLayout): number {
	return getAllSessionIds(layout).length;
}

// Get focused node (first terminal found - could be enhanced with focus tracking)
export function getFirstTerminal(layout: TerminalLayout): TerminalNode | null {
	if (!layout.root) return null;

	function find(node: LayoutNode): TerminalNode | null {
		if (node.type === 'terminal') return node;
		if (node.type === 'split') {
			for (const child of node.children) {
				const found = find(child);
				if (found) return found;
			}
		}
		// webview nodes don't contain terminals
		return null;
	}

	return find(layout.root);
}

// Find the root-level column ID that contains a given node
// Returns the ID of the direct child of root horizontal split that contains nodeId
export function findRootColumnId(layout: TerminalLayout, nodeId: string): string | null {
	if (!layout.root) return null;

	// If root is a terminal or webview and matches, return its id
	if (layout.root.type === 'terminal' || layout.root.type === 'webview') {
		return layout.root.id === nodeId ? layout.root.id : null;
	}

	// Root is a split
	const rootSplit = layout.root;

	// If root is not a horizontal split, the whole thing is one "column"
	if (rootSplit.direction !== 'horizontal') {
		// Check if nodeId is anywhere in this tree
		function contains(node: LayoutNode): boolean {
			if (node.id === nodeId) return true;
			if (node.type === 'split') {
				return node.children.some(contains);
			}
			return false;
		}
		return contains(rootSplit) ? rootSplit.id : null;
	}

	// Root is horizontal split - find which child contains the nodeId
	for (const child of rootSplit.children) {
		function contains(node: LayoutNode): boolean {
			if (node.id === nodeId) return true;
			if (node.type === 'split') {
				return node.children.some(contains);
			}
			return false;
		}
		if (contains(child)) {
			return child.id;
		}
	}

	return null;
}
