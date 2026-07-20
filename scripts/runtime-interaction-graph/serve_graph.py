#!/usr/bin/env python3

from __future__ import annotations

import argparse
import hashlib
import json
import mimetypes
import socket
import threading
import sys
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from types import SimpleNamespace
from urllib.parse import unquote, urlsplit


SCRIPT_DIRECTORY = Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
	sys.path.insert(0, str(SCRIPT_DIRECTORY))
import query_graph


SERVICE_SCHEMA_VERSION = 1
DEFAULT_MAX_BODY_BYTES = 65_536
DEFAULT_MAX_RECORDS = 250
DEFAULT_MAX_TOKENS = 16_000
DEFAULT_MAX_EXPANSIONS = 2_000
DEFAULT_MAX_CONCURRENCY = 8
DEFAULT_SOCKET_TIMEOUT = 10.0
OPERATIONS = ("neighbors", "node", "packs", "paths", "search", "summary")
STATIC_FILES = frozenset({"interaction-graph.html", "interaction-graph.json", "graph-scale.svg",
	"component-graph.json", "query-packs.json", "coverage.json", "completeness.json"})


@dataclass(frozen=True)
class StaticRepresentation:
	path: Path
	size: int
	etag: str


@dataclass(frozen=True)
class StaticAsset:
	content_type: str
	cache_control: str
	identity: StaticRepresentation
	gzip: StaticRepresentation | None


@dataclass(frozen=True)
class ServiceState:
	root: Path
	index: query_graph.GraphIndex
	companions: dict[str, dict | None]
	companion_sha256: dict[str, str]
	component_index: query_graph.GraphIndex | None
	graph_sha256: str
	max_body_bytes: int
	max_records: int
	max_tokens: int
	max_expansions: int
	static_assets: dict[str, StaticAsset]


def static_representation(path: Path, root: Path) -> StaticRepresentation | None:
	if not path.exists():
		return None
	resolved = path.resolve()
	try:
		resolved.relative_to(root)
	except ValueError as error:
		raise query_graph.QueryFailure("invalid-static-artifact",
			"static artifact must stay inside the artifact root") from error
	if not resolved.is_file():
		raise query_graph.QueryFailure("invalid-static-artifact", "static artifact must be a regular file")
	content = resolved.read_bytes()
	if path.name.endswith(".gz") and not content.startswith(b"\x1f\x8b"):
		raise query_graph.QueryFailure("invalid-static-artifact", "compressed static artifact is not gzip data")
	digest = hashlib.sha256(content).hexdigest()
	return StaticRepresentation(resolved, len(content), f'"{digest}"')


def load_static_assets(root: Path) -> dict[str, StaticAsset]:
	assets = {}
	for name in sorted(STATIC_FILES):
		identity = static_representation(root / name, root)
		if identity is None:
			continue
		compressed = static_representation(root / f"{name}.gz", root)
		content_type = mimetypes.guess_type(name)[0] or "application/octet-stream"
		cache_control = "no-cache" if Path(name).suffix.casefold() in {".html", ".json"} \
			else "public, max-age=300, must-revalidate"
		assets[name] = StaticAsset(content_type, cache_control, identity, compressed)
	return assets


def load_state(root: Path, max_body_bytes: int = DEFAULT_MAX_BODY_BYTES,
	max_records: int = DEFAULT_MAX_RECORDS, max_tokens: int = DEFAULT_MAX_TOKENS,
	max_expansions: int = DEFAULT_MAX_EXPANSIONS) -> ServiceState:
	root = root.resolve()
	if not root.is_dir():
		raise query_graph.QueryFailure("invalid-artifact-root", "artifact root must be a directory")
	if max_body_bytes < 1:
		raise query_graph.QueryFailure("invalid-server-limit", "max body bytes must be positive")
	if not 1 <= max_records <= 10_000:
		raise query_graph.QueryFailure("invalid-server-limit", "max records must be from 1 to 10000")
	if not 256 <= max_tokens <= 1_000_000:
		raise query_graph.QueryFailure("invalid-server-limit", "max tokens must be from 256 to 1000000")
	if not 1 <= max_expansions <= 100_000:
		raise query_graph.QueryFailure("invalid-server-limit", "max expansions must be from 1 to 100000")
	graph_path = root / "interaction-graph.json"
	payload = query_graph.load_json(graph_path, dict, "graph")
	index = query_graph.GraphIndex(payload)
	args = SimpleNamespace(graph=graph_path, coverage=None, completeness=None, query_packs=None,
		component_graph=None, no_auto_companions=False)
	companions, companion_sha256 = query_graph.load_companions(args)
	for required in ("coverage", "query_packs", "component_graph"):
		if companions.get(required) is None:
			raise query_graph.QueryFailure("missing-companion", f"required companion is missing: {required}")
	if companions["query_packs"].get("schema_version") != 1:
		raise query_graph.QueryFailure("invalid-query-packs",
			"query-packs.json must use schema_version 1")
	coverage = companions["coverage"]
	for field, actual in (("nodes", len(index.nodes)), ("edges", len(index.edges))):
		if coverage.get(field) != actual:
			raise query_graph.QueryFailure("stale-coverage",
				f"coverage {field} count does not match interaction-graph.json")
	component_index = query_graph.component_index(index, companions.get("component_graph"))
	if component_index is None:
		raise query_graph.QueryFailure("missing-companion", "required companion is missing: component_graph")
	graph_sha256 = hashlib.sha256(graph_path.read_bytes()).hexdigest()
	static_assets = load_static_assets(root)
	missing_assets = sorted(STATIC_FILES - static_assets.keys())
	if missing_assets:
		raise query_graph.QueryFailure("missing-static-artifact",
			f"required static artifact is missing: {missing_assets[0]}")
	return ServiceState(root, index, companions, companion_sha256, component_index, graph_sha256,
		max_body_bytes, max_records, max_tokens, max_expansions, static_assets)


def accepts_gzip(value: str | None) -> bool:
	if not value:
		return False
	qualities = {}
	for part in value.split(","):
		pieces = [piece.strip() for piece in part.split(";")]
		name = pieces[0].casefold()
		if not name:
			continue
		quality = 1.0
		for parameter in pieces[1:]:
			if parameter.casefold().startswith("q="):
				try:
					quality = float(parameter[2:])
				except ValueError:
					quality = 0.0
		if not 0.0 <= quality <= 1.0:
			quality = 0.0
		qualities[name] = quality
	if "gzip" in qualities:
		return qualities["gzip"] > 0
	return qualities.get("*", 0.0) > 0


class GraphHTTPServer(ThreadingHTTPServer):
	daemon_threads = True
	allow_reuse_address = True

	def __init__(self, address: tuple[str, int], state: ServiceState,
		max_concurrency: int = DEFAULT_MAX_CONCURRENCY,
		socket_timeout: float = DEFAULT_SOCKET_TIMEOUT) -> None:
		if max_concurrency < 1:
			raise ValueError("max concurrency must be positive")
		if socket_timeout <= 0:
			raise ValueError("socket timeout must be positive")
		self.state = state
		self.socket_timeout = socket_timeout
		self.request_slots = threading.BoundedSemaphore(max_concurrency)
		super().__init__(address, GraphRequestHandler)

	def get_request(self):
		request, address = super().get_request()
		request.settimeout(self.socket_timeout)
		return request, address

	def process_request(self, request, client_address) -> None:
		self.request_slots.acquire()
		try:
			super().process_request(request, client_address)
		except Exception:
			self.request_slots.release()
			raise

	def process_request_thread(self, request, client_address) -> None:
		try:
			super().process_request_thread(request, client_address)
		finally:
			self.request_slots.release()


class GraphRequestHandler(BaseHTTPRequestHandler):
	server_version = "runtime-graph"
	sys_version = ""

	@property
	def state(self) -> ServiceState:
		return self.server.state

	def version_string(self) -> str:
		return self.server_version

	def log_message(self, format: str, *args: object) -> None:
		return

	def _begin_response(self, status: int, content_type: str, content_length: int,
		cache_control: str = "no-store", content_encoding: str | None = None,
		location: str | None = None, etag: str | None = None) -> None:
		self.send_response(status)
		self.send_header("Content-Type", content_type)
		self.send_header("Content-Length", str(content_length))
		self.send_header("Cache-Control", cache_control)
		self.send_header("Access-Control-Allow-Origin", "*")
		self.send_header("Access-Control-Allow-Methods", "GET, HEAD, POST, OPTIONS")
		self.send_header("Access-Control-Allow-Headers", "Content-Type")
		self.send_header("Cross-Origin-Resource-Policy", "cross-origin")
		self.send_header("Content-Security-Policy",
			"default-src 'none'; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; "
			"img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; "
			"frame-ancestors 'none'; form-action 'none'")
		self.send_header("Permissions-Policy", "camera=(), geolocation=(), microphone=()")
		self.send_header("Referrer-Policy", "no-referrer")
		self.send_header("X-Content-Type-Options", "nosniff")
		self.send_header("X-Frame-Options", "DENY")
		if content_encoding:
			self.send_header("Content-Encoding", content_encoding)
		if etag:
			self.send_header("ETag", etag)
		self.send_header("Vary", "Accept-Encoding")
		if location:
			self.send_header("Location", location)
		self.end_headers()

	def _send_bytes(self, status: int, body: bytes, content_type: str = "application/json; charset=utf-8",
		cache_control: str = "no-store", content_encoding: str | None = None,
		head_only: bool = False) -> None:
		self._begin_response(status, content_type, len(body), cache_control, content_encoding)
		if body and not head_only:
			self.wfile.write(body)

	def _send_json(self, status: int, value: object, head_only: bool = False) -> None:
		body = (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode()
		self._send_bytes(status, body, head_only=head_only)

	def _send_query_failure(self, status: int, failure: query_graph.QueryFailure,
		operation: str = "invalid", parameters: dict | None = None,
		max_records: int | None = None, max_tokens: int | None = None) -> None:
		body = query_graph.error_response(operation, parameters or {}, failure,
			self.state.companions, max_records or self.state.max_records,
			max_tokens or self.state.max_tokens, self.state.graph_sha256,
			self.state.companion_sha256, self.state.index).encode()
		self._send_bytes(status, body + b"\n")

	def send_error(self, code: int, message: str | None = None,
		explain: str | None = None) -> None:
		self._send_json(code, {"error": "http-error", "status": code})

	def do_OPTIONS(self) -> None:
		self._begin_response(204, "text/plain; charset=utf-8", 0)

	def do_GET(self) -> None:
		self._handle_read(False)

	def do_HEAD(self) -> None:
		self._handle_read(True)

	def _handle_read(self, head_only: bool) -> None:
		path = urlsplit(self.path).path
		if path == "/":
			self._begin_response(302, "text/plain; charset=utf-8", 0,
				cache_control="no-store", location="/interaction-graph.html")
			return
		if path == "/healthz":
			self._send_json(200, {"schema_version": SERVICE_SCHEMA_VERSION, "status": "ok"}, head_only)
			return
		if path == "/readyz":
			self._send_json(200, {"schema_version": SERVICE_SCHEMA_VERSION, "status": "ready",
				"graph": {"fingerprint": self.state.graph_sha256,
					"nodes": len(self.state.index.nodes), "edges": len(self.state.index.edges),
					"companions": self.state.companion_sha256}}, head_only)
			return
		if path == "/api/v1":
			self._send_json(200, self._metadata(), head_only)
			return
		if path.startswith("/api/"):
			body = b'{"error":"endpoint-not-found"}\n'
			self._send_bytes(404, body, head_only=head_only)
			return
		self._serve_static(path, head_only)

	def do_POST(self) -> None:
		path = urlsplit(self.path).path
		if path != "/api/v1/query":
			self._send_query_failure(404,
				query_graph.QueryFailure("endpoint-not-found", "API endpoint does not exist"))
			return
		self._handle_query()

	def _metadata(self) -> dict:
		return {
			"schema_version": SERVICE_SCHEMA_VERSION,
			"service": "runtime-interaction-graph",
			"query_response_schema_version": query_graph.SCHEMA_VERSION,
			"operations": list(OPERATIONS),
			"graph": {"schema_version": 2, "fingerprint": self.state.graph_sha256,
				"companions": self.state.companion_sha256,
				"nodes": len(self.state.index.nodes), "edges": len(self.state.index.edges)},
			"limits": {"max_body_bytes": self.state.max_body_bytes,
				"max_records": self.state.max_records, "max_tokens": self.state.max_tokens,
				"max_expansions": self.state.max_expansions},
		}

	def _read_json_request(self) -> dict:
		if self.headers.get_all("Transfer-Encoding"):
			self.close_connection = True
			raise query_graph.QueryFailure("invalid-request", "transfer encoding is not supported")
		if self.headers.get_all("Content-Encoding"):
			self.close_connection = True
			raise query_graph.QueryFailure("invalid-request", "content encoding is not supported")
		content_type = self.headers.get_content_type()
		charset = self.headers.get_content_charset()
		if content_type != "application/json" or \
			(charset is not None and charset.casefold() != "utf-8"):
			raise query_graph.QueryFailure("unsupported-content-type", "request must use application/json UTF-8")
		length_values = self.headers.get_all("Content-Length") or []
		if not length_values:
			raise query_graph.QueryFailure("length-required", "Content-Length is required")
		if len(length_values) != 1 or "," in length_values[0]:
			self.close_connection = True
			raise query_graph.QueryFailure("invalid-content-length", "exactly one Content-Length is required")
		length_value = length_values[0]
		try:
			length = int(length_value)
		except ValueError as error:
			raise query_graph.QueryFailure("invalid-content-length", "Content-Length must be an integer") from error
		if length < 0:
			raise query_graph.QueryFailure("invalid-content-length", "Content-Length must not be negative")
		if length > self.state.max_body_bytes:
			self.close_connection = True
			raise query_graph.QueryFailure("request-body-too-large", "request body exceeds the server limit")
		try:
			body = self.rfile.read(length)
		except (OSError, socket.timeout) as error:
			self.close_connection = True
			raise query_graph.QueryFailure("invalid-request-body", "request body could not be read") from error
		if len(body) != length:
			self.close_connection = True
			raise query_graph.QueryFailure("invalid-request-body", "request body ended before Content-Length")
		def reject_duplicate_keys(pairs: list[tuple[str, object]]) -> dict:
			result = {}
			for key, item in pairs:
				if key in result:
					raise ValueError(f"duplicate key: {key}")
				result[key] = item
			return result
		try:
			value = json.loads(body.decode("utf-8"),
				parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)),
				object_pairs_hook=reject_duplicate_keys)
		except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
			raise query_graph.QueryFailure("invalid-json", "request body must be valid JSON") from error
		if not isinstance(value, dict):
			raise query_graph.QueryFailure("invalid-query", "query request must be a JSON object")
		return value

	def _handle_query(self) -> None:
		request_records = self.state.max_records
		request_tokens = self.state.max_tokens
		operation = "invalid"
		try:
			request = self._read_json_request()
			candidate_records = request.get("max_records", self.state.max_records)
			if isinstance(candidate_records, int) and not isinstance(candidate_records, bool) \
				and 1 <= candidate_records <= self.state.max_records:
				request_records = candidate_records
			candidate_tokens = request.get("max_tokens", self.state.max_tokens)
			if isinstance(candidate_tokens, int) and not isinstance(candidate_tokens, bool) \
				and 256 <= candidate_tokens <= self.state.max_tokens:
				request_tokens = candidate_tokens
			request_records = query_graph.bounded_int(request, "max_records", self.state.max_records,
				1, self.state.max_records)
			request_tokens = query_graph.bounded_int(request, "max_tokens", self.state.max_tokens,
				256, self.state.max_tokens)
			operation_value = request.get("operation")
			if isinstance(operation_value, str):
				operation = operation_value
			if "max_expansions" in request or operation in {"neighbors", "paths"}:
				request["max_expansions"] = query_graph.bounded_int(request, "max_expansions",
					min(DEFAULT_MAX_EXPANSIONS, self.state.max_expansions), 1,
					self.state.max_expansions)
			body = query_graph.execute(self.state.index, self.state.companions, request,
				request_records, request_tokens, self.state.graph_sha256, self.state.component_index,
				companion_sha256=self.state.companion_sha256).encode() + b"\n"
			self._send_bytes(200, body)
		except query_graph.QueryFailure as error:
			status = 413 if error.code == "request-body-too-large" else \
				415 if error.code == "unsupported-content-type" else \
				411 if error.code == "length-required" else 400
			self._send_query_failure(status, error, operation,
				max_records=request_records, max_tokens=request_tokens)
		except Exception:
			self._send_query_failure(500,
				query_graph.QueryFailure("internal-error", "query service failed"), operation,
				max_records=request_records, max_tokens=request_tokens)

	def _serve_static(self, request_path: str, head_only: bool) -> None:
		try:
			decoded = unquote(request_path, errors="strict")
		except UnicodeError:
			self._send_not_found(head_only)
			return
		if not decoded.startswith("/") or "\\" in decoded or "\x00" in decoded:
			self._send_not_found(head_only)
			return
		name = decoded[1:]
		asset = self.state.static_assets.get(name)
		if asset is None:
			self._send_not_found(head_only)
			return
		selected = asset.identity
		content_encoding = None
		if accepts_gzip(self.headers.get("Accept-Encoding")) and asset.gzip is not None:
			selected = asset.gzip
			content_encoding = "gzip"
		if self._etag_matches(selected.etag):
			self._begin_response(304, asset.content_type, 0, asset.cache_control,
				content_encoding, etag=selected.etag)
			return
		try:
			self._begin_response(200, asset.content_type, selected.size, asset.cache_control,
				content_encoding, etag=selected.etag)
			if not head_only:
				with selected.path.open("rb") as source:
					while chunk := source.read(64 * 1024):
						self.wfile.write(chunk)
		except OSError:
			if not self.wfile.closed:
				self.close_connection = True

	def _etag_matches(self, etag: str) -> bool:
		value = self.headers.get("If-None-Match")
		if not value:
			return False
		return any(candidate == "*" or candidate.removeprefix("W/") == etag
			for candidate in (part.strip() for part in value.split(",")))

	def _send_not_found(self, head_only: bool) -> None:
		body = b'{"error":"not-found"}\n'
		self._send_bytes(404, body, head_only=head_only)


def create_server(state: ServiceState, host: str, port: int,
	max_concurrency: int = DEFAULT_MAX_CONCURRENCY,
	socket_timeout: float = DEFAULT_SOCKET_TIMEOUT) -> GraphHTTPServer:
	return GraphHTTPServer((host, port), state, max_concurrency, socket_timeout)


def parser() -> argparse.ArgumentParser:
	result = argparse.ArgumentParser(description="Serve a generated runtime interaction graph")
	result.add_argument("--root", type=Path, required=True,
		help="generated artifact directory containing interaction-graph.json")
	result.add_argument("--host", default="127.0.0.1")
	result.add_argument("--port", type=int, default=8000)
	result.add_argument("--max-body-bytes", type=int, default=DEFAULT_MAX_BODY_BYTES)
	result.add_argument("--max-records", type=int, default=DEFAULT_MAX_RECORDS)
	result.add_argument("--max-tokens", type=int, default=DEFAULT_MAX_TOKENS)
	result.add_argument("--max-expansions", type=int, default=DEFAULT_MAX_EXPANSIONS)
	result.add_argument("--max-concurrency", type=int, default=DEFAULT_MAX_CONCURRENCY)
	result.add_argument("--socket-timeout", type=float, default=DEFAULT_SOCKET_TIMEOUT)
	return result


def main() -> int:
	args = parser().parse_args()
	if not 0 <= args.port <= 65_535:
		parser().error("--port must be from 0 to 65535")
	if args.max_concurrency < 1:
		parser().error("--max-concurrency must be positive")
	if args.socket_timeout <= 0:
		parser().error("--socket-timeout must be positive")
	try:
		state = load_state(args.root, args.max_body_bytes, args.max_records,
			args.max_tokens, args.max_expansions)
	except query_graph.QueryFailure as error:
		parser().error(str(error))
	server = create_server(state, args.host, args.port, args.max_concurrency,
		args.socket_timeout)
	try:
		server.serve_forever()
	except KeyboardInterrupt:
		pass
	finally:
		server.server_close()
	return 0


if __name__ == "__main__":
	raise SystemExit(main())
