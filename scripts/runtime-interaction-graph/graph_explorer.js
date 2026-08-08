"use strict";

const canvas = document.querySelector("#graph-canvas");
const shell = canvas.parentElement;
const context = canvas.getContext("2d", { alpha: false });
const projectionControl = document.querySelector("#projection");
const domainControl = document.querySelector("#domain");
const kindControl = document.querySelector("#edge-kind");
const searchControl = document.querySelector("#search");
const searchResults = document.querySelector("#search-results");
const labelsControl = document.querySelector("#labels");
const inspector = document.querySelector("#inspector");
const legend = document.querySelector("#legend");
const relationCount = document.querySelector("#relation-count");
const projectionNote = document.querySelector("#projection-note");
const liveStatus = document.querySelector("#live-status");
const numberFormat = new Intl.NumberFormat("en-US");
const projectionLabels = {
	execution: "execution",
	callback: "callbacks",
	configuration: "configuration",
	state: "state & invariants",
	asset: "assets & tokens",
	authorization: "authorization",
	"evm-interface": "EVM interfaces",
	deployment: "deployments",
};
const projectionOrder = [
	"execution",
	"callback",
	"configuration",
	"state",
	"asset",
	"authorization",
	"evm-interface",
	"deployment",
];
const availableProjections = [
	...projectionOrder.filter((name) => payload.projections[name]),
	...Object.keys(payload.projections).filter((name) => !projectionOrder.includes(name)),
];
const projectionDescriptions = {
	execution: "Calls, callbacks, dispatch, XCM, EVM, and precompile execution.",
	callback: "Dynamic and runtime-resolved callback relationships.",
	configuration: "Runtime instances, associated bindings, weights, and configuration reads.",
	state: "Storage access, coupled state, guards, and invariants.",
	asset: "Balances, ORML Tokens, ERC-20, pool shares, and routed asset operations.",
	authorization: "Configured origins and privileged entry authorization.",
	"evm-interface": "Selectors, signatures, precompile dispatch, and contract functions.",
	deployment: "Runtime bindings, proxies, deployed contracts, and address references.",
};
const domainColors = {
	frame: "#6f8cff",
	runtime: "#a78bfa",
	"runtime-adapter": "#8b9cff",
	precompile: "#f59e72",
	evm: "#ef7aa8",
	"evm-adapter": "#e879f9",
	"evm-contract": "#fb7185",
	xcm: "#3dd9c5",
	asset: "#f7c65c",
	state: "#65d990",
	authorization: "#c4b5fd",
	dynamic: "#38bdf8",
	unspecified: "#8191a8",
};
const nodes = payload.nodes.map((node) => ({ ...node, x: 0, y: 0, degree: 0, radius: 6 }));
const nodeById = new Map(nodes.map((node) => [node.id, node]));
const edges = payload.edges.map((edge) => ({ ...edge, key: `${edge.source}\u0000${edge.target}`, weight: 0 }));
const edgeKeys = new Set(edges.map((edge) => edge.key));
const apiCache = new Map();
const state = {
	projection: payload.projections.execution?.pairs
		? "execution"
		: availableProjections.find((name) => payload.projections[name].pairs) || availableProjections[0],
	domain: "",
	kind: "",
	selectedNode: null,
	selectedEdge: null,
	matchIds: new Set(),
	showLabels: false,
	zoom: 1,
	panX: 0,
	panY: 0,
};
let activeEdges = [];
let activeNodes = [];
let activeNodeIds = new Set();
let neighborIds = new Set();
let spatial = new Map();
let dpr = 1;
let width = 0;
let height = 0;
let gesture = null;
let apiController = null;

function formatNumber(value) {
	return numberFormat.format(value || 0);
}

function domainColor(domain) {
	if (domainColors[domain]) return domainColors[domain];
	let hash = 2166136261;
	for (const character of domain) hash = Math.imul(hash ^ character.charCodeAt(0), 16777619);
	return `hsl(${Math.abs(hash) % 360} 62% 65%)`;
}

function stableHash(value) {
	let hash = 2166136261;
	for (const character of value) hash = Math.imul(hash ^ character.charCodeAt(0), 16777619);
	return hash >>> 0;
}

function create(tag, className, text) {
	const element = document.createElement(tag);
	if (className) element.className = className;
	if (text !== undefined) element.textContent = String(text);
	return element;
}

function compactId(value, limit = 42) {
	return value.length <= limit ? value : `${value.slice(0, limit - 1)}…`;
}

function edgeWeight(edge) {
	if (state.kind) return edge.kind_counts[state.kind] || 0;
	return edge.projection_counts[state.projection] || 0;
}

function fillProjectionControl() {
	for (const projection of availableProjections) {
		const stats = payload.projections[projection];
		const option = new Option(`${projectionLabels[projection] || projection} · ${formatNumber(stats.nodes)}`, projection);
		projectionControl.add(option);
	}
	projectionControl.value = state.projection;
}

function fillDomainControl() {
	const domains = [...new Set(nodes.map((node) => node.domain || "unspecified"))].sort();
	for (const domain of domains) domainControl.add(new Option(domain, domain));
}

function fillKindControl() {
	const previous = state.kind;
	kindControl.replaceChildren(new Option("all kinds", ""));
	for (const kind of payload.projections[state.projection].kinds) kindControl.add(new Option(kind, kind));
	state.kind = payload.projections[state.projection].kinds.includes(previous) ? previous : "";
	kindControl.value = state.kind;
}

function recompute(fit = true) {
	activeEdges = edges.filter((edge) => {
		if (!edge.projection_counts[state.projection]) return false;
		if (state.kind && !edge.kind_counts[state.kind]) return false;
		if (!state.domain) return true;
		return nodeById.get(edge.source)?.domain === state.domain || nodeById.get(edge.target)?.domain === state.domain;
	});
	activeNodeIds = new Set();
	for (const edge of activeEdges) {
		activeNodeIds.add(edge.source);
		activeNodeIds.add(edge.target);
		edge.weight = edgeWeight(edge);
	}
	activeNodes = nodes.filter((node) => activeNodeIds.has(node.id));
	for (const node of activeNodes) node.degree = 0;
	for (const edge of activeEdges) {
		nodeById.get(edge.source).degree += edge.weight;
		nodeById.get(edge.target).degree += edge.weight;
	}
	for (const node of activeNodes) node.radius = 5 + Math.min(9, Math.log2(node.degree + 1) * 1.6);
	layoutNodes();
	buildSpatialIndex();
	const selectionRemoved = (state.selectedNode && !activeNodeIds.has(state.selectedNode)) ||
		(state.selectedEdge && !activeEdges.includes(state.selectedEdge));
	if (state.selectedNode && !activeNodeIds.has(state.selectedNode)) state.selectedNode = null;
	if (state.selectedEdge && !activeEdges.includes(state.selectedEdge)) state.selectedEdge = null;
	refreshNeighbors();
	renderLegend();
	updateProjectionSummary();
	if (state.selectedNode) renderNodeInspector(nodeById.get(state.selectedNode));
	else if (state.selectedEdge) renderEdgeInspector(state.selectedEdge);
	else if (selectionRemoved) {
		renderEmptyInspector();
		updateHash();
	}
	if (fit) fitGraph();
	else draw();
}

function layoutNodes() {
	const groups = new Map();
	for (const node of activeNodes) {
		const domain = node.domain || "unspecified";
		if (!groups.has(domain)) groups.set(domain, []);
		groups.get(domain).push(node);
	}
	const ordered = [...groups.entries()].sort((left, right) => {
		const count = right[1].length - left[1].length;
		return count || left[0].localeCompare(right[0]);
	});
	const targetWidth = Math.max(1500, Math.sqrt(Math.max(activeNodes.length, 1)) * 72);
	let cursorX = 0;
	let cursorY = 0;
	let rowHeight = 0;
	const golden = Math.PI * (3 - Math.sqrt(5));
	for (const [domain, group] of ordered) {
		group.sort((left, right) => right.degree - left.degree || left.id.localeCompare(right.id));
		const spread = Math.max(120, 20 * Math.sqrt(group.length));
		const clusterWidth = spread * 2 + 150;
		const clusterHeight = spread * 2 + 120;
		if (cursorX && cursorX + clusterWidth > targetWidth) {
			cursorX = 0;
			cursorY += rowHeight + 90;
			rowHeight = 0;
		}
		const centerX = cursorX + clusterWidth / 2;
		const centerY = cursorY + clusterHeight / 2;
		const phase = (stableHash(domain) % 6283) / 1000;
		group.forEach((node, index) => {
			const distance = index ? 19 * Math.sqrt(index) : 0;
			const angle = phase + index * golden;
			node.x = centerX + Math.cos(angle) * distance;
			node.y = centerY + Math.sin(angle) * distance;
		});
		cursorX += clusterWidth + 70;
		rowHeight = Math.max(rowHeight, clusterHeight);
	}
}

function buildSpatialIndex() {
	spatial = new Map();
	for (const node of activeNodes) {
		const key = `${Math.floor(node.x / 100)},${Math.floor(node.y / 100)}`;
		if (!spatial.has(key)) spatial.set(key, []);
		spatial.get(key).push(node);
	}
}

function resizeCanvas() {
	const bounds = shell.getBoundingClientRect();
	width = Math.max(1, bounds.width);
	height = Math.max(1, bounds.height);
	dpr = Math.min(window.devicePixelRatio || 1, 2);
	canvas.width = Math.floor(width * dpr);
	canvas.height = Math.floor(height * dpr);
	canvas.style.width = `${width}px`;
	canvas.style.height = `${height}px`;
	draw();
}

function fitGraph() {
	if (!activeNodes.length || !width || !height) {
		draw();
		return;
	}
	const xs = activeNodes.map((node) => node.x);
	const ys = activeNodes.map((node) => node.y);
	const minX = Math.min(...xs) - 70;
	const maxX = Math.max(...xs) + 70;
	const minY = Math.min(...ys) - 70;
	const maxY = Math.max(...ys) + 70;
	state.zoom = Math.max(.08, Math.min(2.2, .9 * Math.min(width / Math.max(maxX - minX, 1), height / Math.max(maxY - minY, 1))));
	state.panX = width / 2 - ((minX + maxX) / 2) * state.zoom;
	state.panY = height / 2 - ((minY + maxY) / 2) * state.zoom;
	draw();
}

function screenPoint(node) {
	return { x: node.x * state.zoom + state.panX, y: node.y * state.zoom + state.panY };
}

function worldPoint(x, y) {
	return { x: (x - state.panX) / state.zoom, y: (y - state.panY) / state.zoom };
}

function edgePoints(edge) {
	const sourceNode = nodeById.get(edge.source);
	const targetNode = nodeById.get(edge.target);
	const source = screenPoint(sourceNode);
	const target = screenPoint(targetNode);
	const dx = target.x - source.x;
	const dy = target.y - source.y;
	const length = Math.hypot(dx, dy) || 1;
	const ux = dx / length;
	const uy = dy / length;
	const reciprocal = edgeKeys.has(`${edge.target}\u0000${edge.source}`);
	const offset = reciprocal ? (edge.source < edge.target ? 3.5 : -3.5) : 0;
	const sourceInset = Math.min(sourceNode.radius + 1, length * .4);
	const targetInset = Math.min(targetNode.radius + 2, length * .4);
	return {
		sx: source.x + ux * sourceInset - uy * offset,
		sy: source.y + uy * sourceInset + ux * offset,
		tx: target.x - ux * targetInset - uy * offset,
		ty: target.y - uy * targetInset + ux * offset,
	};
}

function drawGrid() {
	const step = 100 * state.zoom;
	if (step < 24) return;
	context.strokeStyle = "rgba(80, 105, 142, .075)";
	context.lineWidth = 1;
	context.beginPath();
	for (let x = ((state.panX % step) + step) % step; x < width; x += step) {
		context.moveTo(x, 0);
		context.lineTo(x, height);
	}
	for (let y = ((state.panY % step) + step) % step; y < height; y += step) {
		context.moveTo(0, y);
		context.lineTo(width, y);
	}
	context.stroke();
}

function drawArrow(points, color) {
	const angle = Math.atan2(points.ty - points.sy, points.tx - points.sx);
	const size = 6;
	context.fillStyle = color;
	context.beginPath();
	context.moveTo(points.tx, points.ty);
	context.lineTo(points.tx - Math.cos(angle - .55) * size, points.ty - Math.sin(angle - .55) * size);
	context.lineTo(points.tx - Math.cos(angle + .55) * size, points.ty - Math.sin(angle + .55) * size);
	context.closePath();
	context.fill();
}

function drawNodeShape(node, point, color, alpha) {
	const radius = node.radius;
	context.globalAlpha = alpha;
	context.fillStyle = color;
	context.beginPath();
	if (node.id.startsWith("boundary:") || node.kind === "execution-boundary") {
		context.moveTo(point.x, point.y - radius - 1);
		context.lineTo(point.x + radius + 1, point.y);
		context.lineTo(point.x, point.y + radius + 1);
		context.lineTo(point.x - radius - 1, point.y);
		context.closePath();
	} else if (node.kind === "precompile") {
		context.rect(point.x - radius, point.y - radius, radius * 2, radius * 2);
	} else if (["deployed-contract", "deployment-alias", "evm-address"].includes(node.kind)) {
		context.moveTo(point.x, point.y - radius - 1);
		context.lineTo(point.x + radius + 1, point.y + radius);
		context.lineTo(point.x - radius - 1, point.y + radius);
		context.closePath();
	} else {
		context.arc(point.x, point.y, radius, 0, Math.PI * 2);
	}
	context.fill();
	context.globalAlpha = 1;
}

function draw() {
	if (!context) return;
	context.setTransform(dpr, 0, 0, dpr, 0, 0);
	context.fillStyle = "#070c16";
	context.fillRect(0, 0, width, height);
	drawGrid();
	const baseEdgeAlpha = activeEdges.length < 100 ? .28 : activeEdges.length < 700 ? .16 : .09;
	for (const edge of activeEdges) {
		const related = state.selectedNode && (edge.source === state.selectedNode || edge.target === state.selectedNode);
		const selected = state.selectedEdge === edge;
		const alpha = selected ? .95 : state.selectedNode ? (related ? .72 : .025) : baseEdgeAlpha;
		const color = selected ? "#f8c15c" : related ? "#75d9ff" : "#51627c";
		const points = edgePoints(edge);
		if (Math.max(points.sx, points.tx) < -30 || Math.min(points.sx, points.tx) > width + 30 ||
			Math.max(points.sy, points.ty) < -30 || Math.min(points.sy, points.ty) > height + 30) continue;
		context.globalAlpha = alpha;
		context.strokeStyle = color;
		context.lineWidth = Math.min(4, .55 + Math.log2(edge.weight + 1) * .55);
		context.beginPath();
		context.moveTo(points.sx, points.sy);
		context.lineTo(points.tx, points.ty);
		context.stroke();
		context.globalAlpha = 1;
		if (selected || related) drawArrow(points, color);
	}
	const labelThreshold = activeNodes.length <= 450 && state.zoom > .55;
	for (const node of activeNodes) {
		const point = screenPoint(node);
		if (point.x < -30 || point.x > width + 30 || point.y < -30 || point.y > height + 30) continue;
		const selected = node.id === state.selectedNode;
		const related = selected || neighborIds.has(node.id);
		const matched = state.matchIds.has(node.id);
		const alpha = state.selectedNode ? (related ? 1 : .14) : 1;
		drawNodeShape(node, point, domainColor(node.domain || "unspecified"), alpha);
		if (selected || matched) {
			context.strokeStyle = selected ? "#fff" : "#f8d86b";
			context.lineWidth = selected ? 2.5 : 2;
			context.beginPath();
			context.arc(point.x, point.y, node.radius + 4, 0, Math.PI * 2);
			context.stroke();
		}
		const show = selected || matched || (state.selectedNode && related) || state.showLabels ||
			(labelThreshold && node.degree >= 2);
		if (!show) continue;
		const label = compactId(node.label === node.id ? node.id : `${node.label} · ${node.id}`, 52);
		context.font = selected ? "600 11px system-ui" : "10px system-ui";
		const measured = context.measureText(label).width;
		context.globalAlpha = Math.max(alpha, .72);
		context.fillStyle = "rgba(7, 12, 22, .88)";
		context.fillRect(point.x + node.radius + 5, point.y - 8, measured + 8, 16);
		context.fillStyle = selected ? "#fff" : "#b8c7dc";
		context.fillText(label, point.x + node.radius + 9, point.y + 3.5);
		context.globalAlpha = 1;
	}
}

function hitNode(x, y) {
	const world = worldPoint(x, y);
	const range = Math.max(1, Math.ceil(18 / state.zoom / 100));
	const cellX = Math.floor(world.x / 100);
	const cellY = Math.floor(world.y / 100);
	let found = null;
	let distance = Infinity;
	for (let dx = -range; dx <= range; dx += 1) {
		for (let dy = -range; dy <= range; dy += 1) {
			for (const node of spatial.get(`${cellX + dx},${cellY + dy}`) || []) {
				const point = screenPoint(node);
				const candidate = Math.hypot(point.x - x, point.y - y);
				if (candidate <= node.radius + 6 && candidate < distance) {
					found = node;
					distance = candidate;
				}
			}
		}
	}
	return found;
}

function segmentDistance(x, y, points) {
	const dx = points.tx - points.sx;
	const dy = points.ty - points.sy;
	const length = dx * dx + dy * dy || 1;
	const position = Math.max(0, Math.min(1, ((x - points.sx) * dx + (y - points.sy) * dy) / length));
	return Math.hypot(x - (points.sx + position * dx), y - (points.sy + position * dy));
}

function hitEdge(x, y) {
	let found = null;
	let distance = 7;
	for (const edge of activeEdges) {
		const candidate = segmentDistance(x, y, edgePoints(edge));
		if (candidate < distance) {
			found = edge;
			distance = candidate;
		}
	}
	return found;
}

function eventPoint(event) {
	const bounds = canvas.getBoundingClientRect();
	return { x: event.clientX - bounds.left, y: event.clientY - bounds.top };
}

function refreshNeighbors() {
	neighborIds = new Set(state.selectedNode ? [state.selectedNode] : []);
	if (!state.selectedNode) return;
	for (const edge of activeEdges) {
		if (edge.source === state.selectedNode) neighborIds.add(edge.target);
		if (edge.target === state.selectedNode) neighborIds.add(edge.source);
	}
}

function metadataList(node) {
	const list = create("dl", "metadata");
	const fields = ["kind", "domain", "activity", "runtime_alias", "entrypoint_kind", "network", "address",
		"chain_address_id", "file", "source", "configured_by", "roles", "semantic_domains"];
	for (const field of fields) {
		if (node[field] === undefined || node[field] === null || node[field] === "") continue;
		list.append(create("dt", "", field.replaceAll("_", " ")),
			create("dd", "", Array.isArray(node[field]) ? node[field].join(", ") : node[field]));
	}
	return list;
}

function nodeButton(node, direction, edge) {
	const button = create("button");
	button.type = "button";
	button.append(create("span", "", direction), create("span", "", compactId(node.id, 48)),
		create("small", "", formatNumber(edge.weight)));
	button.addEventListener("click", () => selectNode(node.id));
	return button;
}

function renderNodeInspector(node) {
	inspector.replaceChildren();
	inspector.append(create("h2", "", node.label), create("code", "node-id", node.id));
	const description = node.semantic_description || node.description;
	if (description) inspector.append(create("p", "description", description));
	const domainPill = create("span", "pill", node.domain || "unspecified");
	domainPill.style.setProperty("--color", domainColor(node.domain || "unspecified"));
	inspector.append(domainPill, create("span", "pill", node.kind), metadataList(node));
	const relationships = activeEdges.filter((edge) => edge.source === node.id || edge.target === node.id)
		.sort((left, right) => right.weight - left.weight || left.key.localeCompare(right.key));
	inspector.append(create("h3", "", `neighbors · ${relationships.length}`));
	const neighborList = create("div", "neighbor-list");
	for (const edge of relationships.slice(0, 80)) {
		const outgoing = edge.source === node.id;
		neighborList.append(nodeButton(nodeById.get(outgoing ? edge.target : edge.source), outgoing ? "→" : "←", edge));
	}
	if (!relationships.length) neighborList.append(create("p", "description", "No relationships in the selected projection."));
	inspector.append(neighborList);
	const details = create("details");
	details.append(create("summary", "", "bounded API metadata"));
	const pre = create("pre", "", "loading…");
	details.append(pre);
	inspector.append(details);
	loadApiNode(node.id, pre);
}

function renderEdgeInspector(edge) {
	inspector.replaceChildren();
	inspector.append(create("h2", "", "Directed relationship"));
	const source = nodeById.get(edge.source);
	const target = nodeById.get(edge.target);
	inspector.append(create("h3", "", "endpoints"));
	const endpoints = create("div", "neighbor-list");
	endpoints.append(nodeButton(source, "from", edge), nodeButton(target, "to", edge));
	inspector.append(endpoints, create("h3", "", `edge kinds · ${Object.keys(edge.kind_counts).length}`));
	const kinds = create("div", "kind-list");
	for (const [kind, count] of Object.entries(edge.kind_counts).sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))) {
		const row = create("div", "kind-row");
		row.append(create("span", "", kind), create("strong", "", formatNumber(count)));
		kinds.append(row);
	}
	inspector.append(kinds);
	const projections = create("p", "description", `Appears in: ${Object.keys(edge.projection_counts).join(", ")}. Aggregated evidence variants: ${formatNumber(edge.count)}.`);
	inspector.append(projections);
	const details = create("details");
	details.open = true;
	details.append(create("summary", "", "representative evidence by kind"),
		create("pre", "", JSON.stringify(edge.samples, null, 2)));
	inspector.append(details);
}

async function loadApiNode(id, target) {
	if (apiCache.has(id)) {
		target.textContent = apiCache.get(id);
		return;
	}
	if (!/^https?:$/.test(location.protocol)) {
		target.textContent = "API drill-down is available when this page is served by the graph container.";
		return;
	}
	apiController?.abort();
	apiController = new AbortController();
	try {
		const response = await fetch("api/v1/query", {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({ operation: "node", id, max_records: 20, max_tokens: 4000 }),
			signal: apiController.signal,
		});
		if (!response.ok) throw new Error(`API returned ${response.status}`);
		const value = await response.json();
		const record = value.result?.records?.[0] || value;
		const encoded = JSON.stringify(record, null, 2);
		apiCache.set(id, encoded);
		target.textContent = encoded;
	} catch (error) {
		if (error.name !== "AbortError") target.textContent = `API metadata unavailable: ${error.message}`;
	}
}

function ensureNodeVisible(node) {
	if (activeNodeIds.has(node.id)) return;
	state.domain = "";
	state.kind = "";
	domainControl.value = "";
	kindControl.value = "";
	const projection = node.projections.includes(state.projection) ? state.projection : node.projections[0];
	if (projection) {
		state.projection = projection;
		projectionControl.value = projection;
		fillKindControl();
	}
	recompute(true);
}

function updateHash(values = {}) {
	const parameters = new URLSearchParams(values);
	history.replaceState(null, "", `${location.pathname}${location.search}${parameters.size ? `#${parameters}` : ""}`);
}

function selectNode(id, setHash = true) {
	const node = nodeById.get(id);
	if (!node) return;
	ensureNodeVisible(node);
	const point = screenPoint(node);
	if (point.x < 0 || point.x > width || point.y < 0 || point.y > height) {
		state.panX = width / 2 - node.x * state.zoom;
		state.panY = height / 2 - node.y * state.zoom;
	}
	state.selectedNode = id;
	state.selectedEdge = null;
	refreshNeighbors();
	renderNodeInspector(node);
	if (setHash) updateHash({ node: id });
	liveStatus.textContent = `Selected ${id} with ${neighborIds.size - 1} visible neighbors.`;
	draw();
}

function selectEdge(edge, setHash = true) {
	state.selectedNode = null;
	state.selectedEdge = edge;
	refreshNeighbors();
	renderEdgeInspector(edge);
	if (setHash) updateHash({ source: edge.source, target: edge.target });
	liveStatus.textContent = `Selected relationship from ${edge.source} to ${edge.target}.`;
	draw();
}

function renderEmptyInspector() {
	const empty = create("div", "empty-inspector");
	empty.append(
		create("span", "", "↗"),
		create("h2", "", "Select a node or relationship"),
		create("p", "", "Neighbors, edge kinds, evidence samples, and bounded API metadata appear here."),
	);
	inspector.replaceChildren(empty);
}

function clearSelection() {
	state.selectedNode = null;
	state.selectedEdge = null;
	refreshNeighbors();
	renderEmptyInspector();
	updateHash();
	draw();
}

function searchRank(node, query) {
	const id = node.id.toLowerCase();
	const label = String(node.label).toLowerCase();
	if (id === query || label === query) return 0;
	if (id.startsWith(query) || label.startsWith(query)) return 1;
	if (id.includes(query) || label.includes(query)) return 2;
	const metadata = [node.kind, node.domain, node.runtime_alias, node.network, ...(node.names || [])].join(" ").toLowerCase();
	return metadata.includes(query) ? 3 : Infinity;
}

function runSearch() {
	const query = searchControl.value.trim().toLowerCase();
	searchResults.replaceChildren();
	if (!query) {
		state.matchIds = new Set();
		searchResults.classList.remove("open");
		draw();
		return [];
	}
	const matches = nodes.map((node) => ({ node, rank: searchRank(node, query) }))
		.filter((item) => Number.isFinite(item.rank))
		.sort((left, right) => left.rank - right.rank || right.node.degree - left.node.degree || left.node.id.localeCompare(right.node.id));
	state.matchIds = new Set(matches.slice(0, 100).map((item) => item.node.id));
	for (const { node } of matches.slice(0, 16)) {
		const button = create("button");
		const dot = create("span", "search-dot");
		dot.style.setProperty("--color", domainColor(node.domain || "unspecified"));
		button.append(dot, create("span", "", node.id), create("small", "", node.kind));
		button.type = "button";
		button.addEventListener("click", () => {
			selectNode(node.id);
			searchResults.classList.remove("open");
		});
		searchResults.append(button);
	}
	if (!matches.length) searchResults.append(create("span", "description", "No matching nodes."));
	searchResults.classList.add("open");
	draw();
	return matches;
}

function renderLegend() {
	legend.replaceChildren();
	const counts = new Map();
	for (const node of activeNodes) counts.set(node.domain || "unspecified", (counts.get(node.domain || "unspecified") || 0) + 1);
	for (const [domain, count] of [...counts.entries()].sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))) {
		const button = create("button", state.domain === domain ? "active" : "");
		button.type = "button";
		const swatch = create("span", "swatch");
		swatch.style.setProperty("--color", domainColor(domain));
		button.append(swatch, create("span", "", `${domain} · ${formatNumber(count)}`));
		button.addEventListener("click", () => {
			state.domain = state.domain === domain ? "" : domain;
			domainControl.value = state.domain;
			recompute(true);
		});
		legend.append(button);
	}
}

function updateProjectionSummary() {
	const evidence = activeEdges.reduce((total, edge) => total + edge.weight, 0);
	projectionNote.textContent = projectionDescriptions[state.projection] || "Evidence-backed component relationships.";
	relationCount.textContent = `${formatNumber(activeNodes.length)} nodes · ${formatNumber(activeEdges.length)} pairs · ${formatNumber(evidence)} evidence edges`;
	liveStatus.textContent = relationCount.textContent;
}

function zoomAt(factor, x = width / 2, y = height / 2) {
	const world = worldPoint(x, y);
	state.zoom = Math.max(.05, Math.min(5, state.zoom * factor));
	state.panX = x - world.x * state.zoom;
	state.panY = y - world.y * state.zoom;
	draw();
}

canvas.addEventListener("pointerdown", (event) => {
	const point = eventPoint(event);
	const node = hitNode(point.x, point.y);
	gesture = { type: node ? "node" : "pan", node, startX: point.x, startY: point.y,
		lastX: point.x, lastY: point.y, moved: false };
	canvas.setPointerCapture(event.pointerId);
});

canvas.addEventListener("pointermove", (event) => {
	const point = eventPoint(event);
	if (!gesture) {
		canvas.style.cursor = hitNode(point.x, point.y) ? "pointer" : "grab";
		return;
	}
	if (Math.hypot(point.x - gesture.startX, point.y - gesture.startY) > 3) gesture.moved = true;
	if (gesture.type === "node" && gesture.moved) {
		const world = worldPoint(point.x, point.y);
		gesture.node.x = world.x;
		gesture.node.y = world.y;
		buildSpatialIndex();
	} else if (gesture.type === "pan") {
		state.panX += point.x - gesture.lastX;
		state.panY += point.y - gesture.lastY;
	}
	gesture.lastX = point.x;
	gesture.lastY = point.y;
	draw();
});

canvas.addEventListener("pointerup", (event) => {
	const point = eventPoint(event);
	if (gesture && !gesture.moved) {
		if (gesture.node) selectNode(gesture.node.id);
		else {
			const edge = hitEdge(point.x, point.y);
			if (edge) selectEdge(edge);
		}
	}
	gesture = null;
	canvas.releasePointerCapture(event.pointerId);
});

canvas.addEventListener("pointercancel", () => {
	gesture = null;
});

canvas.addEventListener("wheel", (event) => {
	event.preventDefault();
	const point = eventPoint(event);
	zoomAt(Math.exp(-event.deltaY * .0012), point.x, point.y);
}, { passive: false });

canvas.addEventListener("dblclick", (event) => {
	const point = eventPoint(event);
	const node = hitNode(point.x, point.y);
	if (!node) return;
	selectNode(node.id);
	state.panX = width / 2 - node.x * state.zoom;
	state.panY = height / 2 - node.y * state.zoom;
	draw();
});

canvas.addEventListener("keydown", (event) => {
	if (event.key === "Escape") clearSelection();
	else if (event.key === "0") fitGraph();
	else if (["+", "="].includes(event.key)) zoomAt(1.2);
	else if (event.key === "-") zoomAt(1 / 1.2);
	else if (event.key === "ArrowLeft") state.panX += 35;
	else if (event.key === "ArrowRight") state.panX -= 35;
	else if (event.key === "ArrowUp") state.panY += 35;
	else if (event.key === "ArrowDown") state.panY -= 35;
	else return;
	event.preventDefault();
	draw();
});

projectionControl.addEventListener("change", () => {
	state.projection = projectionControl.value;
	fillKindControl();
	recompute(true);
});

domainControl.addEventListener("change", () => {
	state.domain = domainControl.value;
	recompute(true);
});

kindControl.addEventListener("change", () => {
	state.kind = kindControl.value;
	recompute(true);
});

labelsControl.addEventListener("change", () => {
	state.showLabels = labelsControl.checked;
	draw();
});

searchControl.addEventListener("input", runSearch);
searchControl.addEventListener("keydown", (event) => {
	if (event.key === "Enter") {
		const first = runSearch()[0];
		if (first) selectNode(first.node.id);
		searchResults.classList.remove("open");
	} else if (event.key === "Escape") {
		searchResults.classList.remove("open");
	}
});

document.addEventListener("pointerdown", (event) => {
	if (!event.target.closest(".search-field")) searchResults.classList.remove("open");
});
document.querySelector("#fit").addEventListener("click", fitGraph);
document.querySelector("#reset-selection").addEventListener("click", clearSelection);

function restoreHash() {
	const parameters = new URLSearchParams(location.hash.slice(1));
	const node = parameters.get("node");
	if (nodeById.has(node)) {
		selectNode(node, false);
		return;
	}
	const source = parameters.get("source");
	const target = parameters.get("target");
	const edge = edges.find((candidate) => candidate.source === source && candidate.target === target);
	if (edge) {
		const projection = Object.keys(edge.projection_counts)[0];
		if (projection && projection !== state.projection) {
			state.projection = projection;
			projectionControl.value = projection;
			fillKindControl();
			recompute(true);
		}
		selectEdge(edge, false);
	}
}

document.querySelector("#full-node-count").textContent = formatNumber(payload.summary.nodes.raw);
document.querySelector("#full-edge-count").textContent = formatNumber(payload.summary.edges.raw);
document.querySelector("#component-count").textContent = formatNumber(payload.summary.components.raw);
fillProjectionControl();
fillDomainControl();
fillKindControl();
resizeCanvas();
recompute(true);
restoreHash();
if ("ResizeObserver" in window) new ResizeObserver(resizeCanvas).observe(shell);
else window.addEventListener("resize", resizeCanvas);
