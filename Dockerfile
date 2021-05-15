FROM rust:latest as builder

WORKDIR /build

COPY . /build

RUN cargo build --release

FROM debian:buster-slim
COPY --from=builder /build/target/release/hydra-dx /usr/local/bin

RUN useradd -m -u 1000 -U -s /bin/sh -d /hydra hydra && \
	mkdir -p /hydra/.local/share && \
	mkdir /data && \
	chown -R hydra:hydra /data && \
	ln -s /data /hydra/.local/share/hydra-dx && \
	rm -rf /usr/bin /usr/sbin

USER hydra
EXPOSE 30333 9933 9944
VOLUME ["/data"]

CMD ["/usr/local/bin/hydra-dx","--chain","lerna"]
