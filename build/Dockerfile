# Build Hydra in builder container
FROM paritytech/ci-linux:production as builder

ENV DEBIAN_FRONTEND=noninteractive

RUN apt update && apt install -y cmake pkg-config libssl-dev git build-essential clang libclang-dev curl libz-dev

ARG UPSTREAM_VERSION

WORKDIR /hydra-dx-node

RUN git clone -b ${UPSTREAM_VERSION} https://github.com/galacticcouncil/HydraDX-node.git .

RUN cargo build --release

# Pull HydraDX into thte production container from the builder
FROM paritytech/ci-linux:production

COPY --from=builder /hydra-dx-node /hydra-dx-node

WORKDIR /hydra-dx-node

ENTRYPOINT ./target/release/hydra-dx --chain lerna --ws-external