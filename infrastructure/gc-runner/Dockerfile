# jammy base on purpose: binaries built here run on toad (glibc < 2.38)
FROM ghcr.io/actions/actions-runner:2.311.0
ARG RUNNER_VERSION=2.338.0
RUN sudo apt update -y && sudo apt install build-essential clang git make curl protobuf-compiler libssl-dev m4 -y
RUN sudo rm -rf bin externals \
 && curl -fsSL https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-linux-x64-${RUNNER_VERSION}.tar.gz | sudo tar xz \
 && sudo chown -R runner:docker /home/runner
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
COPY runner.sh .
CMD ["./runner.sh"]
