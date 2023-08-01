#!/bin/bash

# second update fixes some package errors
sudo apt-get update
sudo apt-get update

sudo apt-get install --assume-yes build-essential
sudo apt-get install --assume-yes jq ca-certificates gnupg pkg-config m4 git clang curl libssl-dev llvm libudev-dev make protobuf-compiler

# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
$HOME/.cargo/bin/cargo install cargo-tarpaulin

# Docker installation
sudo install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
sudo chmod a+r /etc/apt/keyrings/docker.gpg

echo \
  "deb [arch="$(dpkg --print-architecture)" signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \
  "$(. /etc/os-release && echo "$VERSION_CODENAME")" stable" | \
  sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

sudo apt-get update

sudo apt-get install --assume-yes docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin

# Github Action Runner installation
curl -o actions-runner-linux-x64-2.277.1.tar.gz -L https://github.com/actions/runner/releases/download/v2.277.1/actions-runner-linux-x64-2.277.1.tar.gz

tar xzf ./actions-runner-linux-x64-2.277.1.tar.gz
