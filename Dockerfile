FROM ubuntu:22.04
LABEL org.opencontainers.image.source = "https://github.com/galacticcouncil/HydraDX-node"

RUN useradd -m -u 1000 -U -s /bin/sh -d /hydra hydra && \
	mkdir -p /hydra/.local/share && \
	chown -R hydra:hydra /hydra

ADD target/release/hydradx /usr/local/bin/hydradx

RUN chmod +x /usr/local/bin/hydradx

USER hydra
EXPOSE 30333 9933 9944 9615
VOLUME ["/hydra/.local/share"]

ENTRYPOINT ["/usr/local/bin/hydradx"]
CMD ["--prometheus-external", "--", "--execution=wasm" ,"--telemetry-url", "wss://telemetry.hydradx.io:9000/submit/ 0"]
