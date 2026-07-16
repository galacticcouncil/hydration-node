import gzip
import http.client
import json
import socket
import sys
import tempfile
import threading
import unittest
from pathlib import Path


SCRIPT_DIRECTORY = Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
	sys.path.insert(0, str(SCRIPT_DIRECTORY))
import query_graph
import serve_graph


class GraphServerTest(unittest.TestCase):
	def setUp(self) -> None:
		self.temporary = tempfile.TemporaryDirectory()
		self.root = Path(self.temporary.name)
		graph = {
			"schema_version": 2,
			"nodes": [
				{"id": "component:a", "kind": "component", "runtime_active": True},
				{"id": "component:b", "kind": "component", "runtime_active": True},
			],
			"edges": [{"source": "component:a", "target": "component:b", "kind": "calls"}],
		}
		self._write_json("interaction-graph.json", graph)
		self._write_json("component-graph.json", {
			"schema_version": 2,
			"projection": "execution",
			"edges": graph["edges"],
		})
		self._write_json("coverage.json", {"nodes": 2, "edges": 1, "unresolved_targets": 0})
		self._write_json("completeness.json", {"schema_version": 1})
		self._write_json("query-packs.json", {"schema_version": 1, "sample": []})
		self.root.joinpath("interaction-graph.html").write_text("<!doctype html><title>graph</title>")
		self.root.joinpath("graph-scale.svg").write_text("<svg xmlns='http://www.w3.org/2000/svg'/>")
		html = self.root.joinpath("interaction-graph.html").read_bytes()
		self.root.joinpath("interaction-graph.html.gz").write_bytes(gzip.compress(html, mtime=0))
		self.state = serve_graph.load_state(self.root, max_body_bytes=256,
			max_records=20, max_tokens=2_000, max_expansions=50)
		self.server = serve_graph.create_server(self.state, "127.0.0.1", 0,
			max_concurrency=2, socket_timeout=0.5)
		self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
		self.thread.start()

	def tearDown(self) -> None:
		self.server.shutdown()
		self.server.server_close()
		self.thread.join(timeout=2)
		self.temporary.cleanup()

	def _write_json(self, name: str, value: object) -> None:
		self.root.joinpath(name).write_text(json.dumps(value, sort_keys=True))

	def request(self, method: str, path: str, body: bytes | None = None,
		headers: dict[str, str] | None = None) -> tuple[int, dict[str, str], bytes]:
		connection = http.client.HTTPConnection(*self.server.server_address, timeout=2)
		connection.request(method, path, body=body, headers=headers or {})
		response = connection.getresponse()
		data = response.read()
		result = response.status, {key.casefold(): value for key, value in response.getheaders()}, data
		connection.close()
		return result

	def raw_request(self, request: bytes) -> bytes:
		with socket.create_connection(self.server.server_address, timeout=2) as connection:
			connection.sendall(request)
			connection.shutdown(socket.SHUT_WR)
			chunks = []
			while chunk := connection.recv(64 * 1024):
				chunks.append(chunk)
		return b"".join(chunks)

	def test_health_readiness_metadata_and_root(self) -> None:
		status, headers, body = self.request("GET", "/")
		self.assertEqual(status, 302)
		self.assertEqual(headers["location"], "/interaction-graph.html")
		status, _, body = self.request("GET", "/healthz")
		self.assertEqual(status, 200)
		self.assertEqual(json.loads(body)["status"], "ok")
		status, _, body = self.request("GET", "/readyz")
		ready = json.loads(body)
		self.assertEqual(status, 200)
		self.assertEqual(ready["graph"]["nodes"], 2)
		self.assertEqual(set(ready["graph"]["companions"]),
			{"coverage", "completeness", "query_packs", "component_graph"})
		status, _, body = self.request("GET", "/api/v1")
		metadata = json.loads(body)
		self.assertEqual(status, 200)
		self.assertEqual(metadata["limits"]["max_records"], 20)
		self.assertIn("paths", metadata["operations"])

	def test_static_gzip_head_etag_and_conditional_get(self) -> None:
		status, identity_headers, identity = self.request("GET", "/interaction-graph.html",
			headers={"Accept-Encoding": "gzip;q=0, *;q=0"})
		self.assertEqual(status, 200)
		self.assertNotIn("content-encoding", identity_headers)
		self.assertEqual(identity, self.root.joinpath("interaction-graph.html").read_bytes())
		self.assertIn("etag", identity_headers)

		status, gzip_headers, compressed = self.request("GET", "/interaction-graph.html",
			headers={"Accept-Encoding": "br, gzip;q=0.8"})
		self.assertEqual(status, 200)
		self.assertEqual(gzip_headers["content-encoding"], "gzip")
		self.assertEqual(gzip.decompress(compressed), identity)
		self.assertNotEqual(gzip_headers["etag"], identity_headers["etag"])

		status, head_headers, head_body = self.request("HEAD", "/interaction-graph.html",
			headers={"Accept-Encoding": "gzip"})
		self.assertEqual(status, 200)
		self.assertEqual(head_body, b"")
		self.assertEqual(head_headers["content-length"], str(len(compressed)))

		status, conditional_headers, conditional_body = self.request("GET", "/interaction-graph.html",
			headers={"Accept-Encoding": "gzip", "If-None-Match": gzip_headers["etag"]})
		self.assertEqual(status, 304)
		self.assertEqual(conditional_headers["etag"], gzip_headers["etag"])
		self.assertEqual(conditional_body, b"")

	def test_static_routes_are_an_explicit_allowlist(self) -> None:
		self.root.joinpath("secret.txt").write_text("not public")
		for path in ("/secret.txt", "/../secret.txt", "/%2e%2e/secret.txt",
			"/%252e%252e/secret.txt", "/interaction-graph.html/extra", "/.hidden"):
			with self.subTest(path=path):
				status, _, body = self.request("GET", path)
				self.assertEqual(status, 404)
				self.assertEqual(json.loads(body)["error"], "not-found")
		status, _, body = self.request("HEAD", "/secret.txt")
		self.assertEqual(status, 404)
		self.assertEqual(body, b"")

	def test_query_endpoint_and_server_side_limits(self) -> None:
		request = json.dumps({"operation": "summary", "max_records": 5, "max_tokens": 1_000}).encode()
		status, headers, body = self.request("POST", "/api/v1/query", request,
			headers={"Content-Type": "application/json; charset=utf-8"})
		response = json.loads(body)
		self.assertEqual(status, 200)
		self.assertEqual(headers["access-control-allow-origin"], "*")
		self.assertEqual(response["schema_version"], 1)
		self.assertEqual(response["query"]["operation"], "summary")

		request = json.dumps({"operation": "summary", "max_records": 21}).encode()
		status, _, body = self.request("POST", "/api/v1/query", request,
			headers={"Content-Type": "application/json"})
		self.assertEqual(status, 400)
		self.assertEqual(json.loads(body)["result"]["metadata"]["error"], "invalid-query")

	def test_post_rejects_unsupported_or_ambiguous_framing(self) -> None:
		body = b'{"operation":"summary"}'
		status, _, response = self.request("POST", "/api/v1/query", body,
			headers={"Content-Type": "text/plain"})
		self.assertEqual(status, 415)
		self.assertEqual(json.loads(response)["result"]["metadata"]["error"],
			"unsupported-content-type")

		status, _, response = self.request("POST", "/api/v1/query", body,
			headers={"Content-Type": "application/json", "Content-Encoding": "gzip"})
		self.assertEqual(status, 400)
		self.assertEqual(json.loads(response)["result"]["metadata"]["error"], "invalid-request")

		response = self.raw_request(
			b"POST /api/v1/query HTTP/1.1\r\nHost: localhost\r\n"
			b"Content-Type: application/json\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}")
		self.assertIn(b" 400 ", response.split(b"\r\n", 1)[0])
		response = self.raw_request(
			b"POST /api/v1/query HTTP/1.1\r\nHost: localhost\r\n"
			b"Content-Type: application/json\r\nContent-Length: 300\r\n\r\n{}")
		self.assertIn(b" 413 ", response.split(b"\r\n", 1)[0])
		response = self.raw_request(
			b"POST /api/v1/query HTTP/1.1\r\nHost: localhost\r\n"
			b"Content-Type: application/json\r\nContent-Length: 20\r\n\r\n{}")
		self.assertIn(b" 400 ", response.split(b"\r\n", 1)[0])

	def test_load_state_rejects_stale_or_missing_artifacts(self) -> None:
		self._write_json("coverage.json", {"nodes": 2, "edges": 999})
		with self.assertRaisesRegex(query_graph.QueryFailure, "coverage edges count"):
			serve_graph.load_state(self.root)
		self._write_json("coverage.json", {"nodes": 2, "edges": 1})
		self.root.joinpath("graph-scale.svg").unlink()
		with self.assertRaisesRegex(query_graph.QueryFailure, "required static artifact"):
			serve_graph.load_state(self.root)

	def test_accepts_gzip_honors_quality(self) -> None:
		self.assertTrue(serve_graph.accepts_gzip("br, gzip;q=0.5"))
		self.assertTrue(serve_graph.accepts_gzip("*;q=1"))
		self.assertFalse(serve_graph.accepts_gzip("gzip;q=0, *;q=0"))
		self.assertFalse(serve_graph.accepts_gzip("gzip;q=0, *;q=1"))
		self.assertFalse(serve_graph.accepts_gzip("gzip;q=invalid"))
		self.assertFalse(serve_graph.accepts_gzip("gzip;q=2"))


if __name__ == "__main__":
	unittest.main()
