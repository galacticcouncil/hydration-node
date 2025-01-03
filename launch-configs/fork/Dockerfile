FROM node:18-bookworm

RUN apt-get update && apt-get install -y \
    curl \
    wget \
    nodejs \
    npm \
    ca-certificates \
    && update-ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /zombienet/hydration-node/launch-configs/fork

RUN wget -q https://github.com/paritytech/zombienet/releases/latest/download/zombienet-linux-x64 -O /usr/local/bin/zombienet && \
    chmod +x /usr/local/bin/zombienet
RUN zombienet version

RUN mkdir -p /zombienet/polkadot-sdk/target/release
RUN for binary in polkadot polkadot-execute-worker polkadot-prepare-worker; do \
    wget -q https://github.com/paritytech/polkadot-sdk/releases/latest/download/$binary \
        -O /zombienet/polkadot-sdk/target/release/$binary \
    && chmod +x /zombienet/polkadot-sdk/target/release/$binary \
    && /zombienet/polkadot-sdk/target/release/$binary --version; \
    done

RUN mkdir -p /zombienet/hydration-node/target/release
RUN wget -q https://github.com/galacticcouncil/hydration-node/releases/latest/download/hydradx -O /zombienet/hydration-node/target/release/hydradx && \
    chmod +x /zombienet/hydration-node/target/release/hydradx
RUN /zombienet/hydration-node/target/release/hydradx --version

COPY . .

RUN npm i

VOLUME /zombienet/hydration-node/launch-configs/fork/data
EXPOSE 9988

ENV STATE_SOURCE=https://snapshots.play.hydration.cloud/state.json

CMD npm start
