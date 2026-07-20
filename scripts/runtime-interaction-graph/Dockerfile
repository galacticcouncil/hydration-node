# syntax=docker/dockerfile:1.7

ARG PYTHON_IMAGE=python:3.14-alpine@sha256:26730869004e2b9c4b9ad09cab8625e81d256d1ce97e72df5520e806b1709f92
FROM ${PYTHON_IMAGE}

ARG PYTHON_IMAGE
ARG BUILD_DATE=unknown
ARG VCS_REF=unknown
ARG VERSION=dev
ARG GRAPH_SHA256=unknown
ARG RUNTIME_SPEC_VERSION=unknown

LABEL org.opencontainers.image.title="Hydration runtime interaction graph" \
	org.opencontainers.image.description="Static graph dashboard and deterministic query API" \
	org.opencontainers.image.created="${BUILD_DATE}" \
	org.opencontainers.image.revision="${VCS_REF}" \
	org.opencontainers.image.version="${VERSION}" \
	org.opencontainers.image.source="https://github.com/galacticcouncil/hydration-node" \
	org.opencontainers.image.licenses="Apache-2.0" \
	org.opencontainers.image.base.name="${PYTHON_IMAGE}" \
	io.hydration.graph.sha256="${GRAPH_SHA256}" \
	io.hydration.runtime.spec-version="${RUNTIME_SPEC_VERSION}"

ENV PYTHONDONTWRITEBYTECODE=1 \
	PYTHONUNBUFFERED=1

RUN addgroup -S -g 10001 graph \
	&& adduser -S -D -H -u 10001 -G graph graph

WORKDIR /app

COPY --chown=10001:10001 scripts/runtime-interaction-graph/query_graph.py \
	scripts/runtime-interaction-graph/serve_graph.py \
	scripts/runtime-interaction-graph/query-response.schema.json \
	/app/

COPY --from=graph --chown=10001:10001 \
	interaction-graph.json \
	interaction-graph.html \
	graph-scale.svg \
	component-graph.json \
	query-packs.json \
	coverage.json \
	completeness.json \
	/srv/graph/

RUN python -c 'import gzip,pathlib; root=pathlib.Path("/srv/graph"); [path.with_name(path.name + ".gz").write_bytes(gzip.compress(path.read_bytes(), compresslevel=9, mtime=0)) for path in sorted(root.iterdir()) if path.is_file() and path.suffix in {".html", ".json", ".svg"} and path.stat().st_size >= 65536]'

USER 10001:10001

EXPOSE 8080
STOPSIGNAL SIGTERM

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
	CMD ["python", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8080/healthz', timeout=2).read()"]

CMD ["python", "/app/serve_graph.py", "--root", "/srv/graph", "--host", "0.0.0.0", "--port", "8080"]
