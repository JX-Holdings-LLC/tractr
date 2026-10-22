# A Linux development container with everything scripts/dev-setup.sh installs: the system
# packages the player links, Rust (rust-toolchain.toml), the .NET SDK (global.json), Node 22 and
# the extra CI tools. Used by .devcontainer/devcontainer.json (VS Code Dev Containers, Codespaces).
FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

ARG USERNAME=vscode
COPY rust-toolchain.toml global.json Cargo.lock /tmp/repo/
COPY scripts/dev-setup.sh /tmp/repo/scripts/
COPY tools/ci/install-rust.sh /tmp/repo/tools/ci/

RUN apt-get update \
    && apt-get install -y --no-install-recommends python3 \
    && rm -rf /var/lib/apt/lists/*

USER ${USERNAME}
# --full builds wasm-bindgen-cli and cargo-about from source, which takes a while once.
RUN /tmp/repo/scripts/dev-setup.sh --yes --full \
    && rm -rf ~/.cargo/registry/cache ~/.cargo/registry/src

ENV PATH=/home/${USERNAME}/.cargo/bin:/home/${USERNAME}/.dotnet:/home/${USERNAME}/.dotnet/tools:${PATH} \
    DOTNET_ROOT=/home/${USERNAME}/.dotnet \
    DOTNET_CLI_TELEMETRY_OPTOUT=1 \
    DOTNET_NOLOGO=1
