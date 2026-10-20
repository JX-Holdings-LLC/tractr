#!/usr/bin/env bash
# Sets up a Linux or macOS machine to build, test and run everything in this repository, then
# checks the result. Safe to run again: it installs only what is missing or at the wrong version.
#
#   scripts/dev-setup.sh            the core tools: system packages, Rust, the .NET SDK, dotnet tools
#   scripts/dev-setup.sh --full     also what the extra CI jobs use: wasm-bindgen-cli, cargo-about
#   scripts/dev-setup.sh --check    install nothing; report what is present and what is missing
#   scripts/dev-setup.sh --yes      do not ask before running apt-get or brew
#
# Everything is pinned by the repository (rust-toolchain.toml, global.json, dotnet-tools.json,
# Cargo.lock) and downloads are checked: rustup through tools/ci/install-rust.sh (sha256), the .NET
# SDK against the sha512 in Microsoft's release metadata. Nothing is piped into a shell. The .NET
# SDK goes to ~/.dotnet unless a `dotnet` with the right SDK is already on PATH.
# Windows: see scripts/dev-setup.ps1.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mode=install
full=0
assume_yes=0
for arg in "$@"; do
  case "$arg" in
    --check) mode=check ;;
    --full) full=1 ;;
    --yes | -y) assume_yes=1 ;;
    -h | --help)
      sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "unknown option: $arg (see --help)" >&2
      exit 2
      ;;
  esac
done

os="$(uname -s)"
arch="$(uname -m)"
missing=()

say() { printf '\033[1m==>\033[0m %s\n' "$*"; }
ok() { printf '  \033[32mok\033[0m      %s\n' "$*"; }
miss() {
  printf '  \033[31mmissing\033[0m %s\n' "$*"
  missing+=("$1")
}

confirm() {
  [[ $assume_yes == 1 ]] && return 0
  [[ -t 0 ]] || return 0
  read -r -p "$1 [Y/n] " answer
  [[ -z $answer || $answer =~ ^[Yy] ]]
}

# --- Versions the repository pins -----------------------------------------------------------

dotnet_sdk="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["sdk"]["version"])' "$root/global.json")"
rust_channel="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$root/rust-toolchain.toml")"
wasm_bindgen="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/"/, "", $3); print $3; exit }' "$root/Cargo.lock")"
cargo_about=0.9.2

export PATH="$HOME/.cargo/bin:$HOME/.dotnet:$HOME/.dotnet/tools:$PATH"
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1

# --- System packages ------------------------------------------------------------------------

apt_packages=(build-essential curl ca-certificates python3 pkg-config libasound2-dev libudev-dev
  libxkbcommon-x11-0 libfontconfig1 libice6 libsm6 xvfb mesa-vulkan-drivers)

system_packages() {
  say "System packages"
  case "$os" in
    Linux)
      if ! command -v dpkg-query > /dev/null; then
        echo "  not a Debian or Ubuntu system: install the equivalents of ${apt_packages[*]}"
        return
      fi
      local need=()
      for p in "${apt_packages[@]}"; do
        if dpkg-query -W -f='${Status}' "$p" 2> /dev/null | grep -q 'install ok installed'; then
          ok "$p"
        else
          need+=("$p")
        fi
      done
      [[ ${#need[@]} == 0 ]] && return
      if [[ $mode == check ]]; then
        for p in "${need[@]}"; do miss "$p"; done
        return
      fi
      local sudo=()
      [[ $(id -u) != 0 ]] && sudo=(sudo)
      if confirm "Install ${need[*]} with apt-get?"; then
        "${sudo[@]}" apt-get update
        "${sudo[@]}" apt-get install -y --no-install-recommends "${need[@]}"
      else
        for p in "${need[@]}"; do miss "$p"; done
      fi
      ;;
    Darwin)
      # The player needs no extra libraries on macOS (CoreAudio, IOKit). Only the toolchain.
      if xcode-select -p > /dev/null 2>&1; then ok "Xcode command line tools"; else miss "Xcode command line tools (xcode-select --install)"; fi
      command -v python3 > /dev/null && ok python3 || miss python3
      ;;
  esac
}

# --- Rust -----------------------------------------------------------------------------------

rust() {
  say "Rust $rust_channel"
  if ! command -v rustup > /dev/null; then
    if [[ $mode == check ]]; then
      miss rustup
      return
    fi
    if [[ $os == Linux && $arch == x86_64 ]]; then
      "$root/tools/ci/install-rust.sh"
    else
      echo "  install rustup from https://rustup.rs, then run this script again"
      miss rustup
      return
    fi
  fi
  # rustup installs the toolchain, components and targets rust-toolchain.toml names on first use.
  if [[ $mode == install ]]; then
    (cd "$root" && rustup toolchain install > /dev/null)
  fi
  local have
  have="$(cd "$root" && rustc --version 2> /dev/null || true)"
  if [[ $have == "rustc $rust_channel "* ]]; then ok "$have"; else miss "rustc $rust_channel (have: ${have:-none})"; fi
}

# --- .NET -----------------------------------------------------------------------------------

has_sdk() { command -v dotnet > /dev/null && dotnet --list-sdks 2> /dev/null | grep -q "^$dotnet_sdk "; }

dotnet_sdk_install() {
  local rid
  case "$os-$arch" in
    Linux-x86_64) rid=linux-x64 ;;
    Linux-aarch64) rid=linux-arm64 ;;
    Darwin-x86_64) rid=osx-x64 ;;
    Darwin-arm64) rid=osx-arm64 ;;
    *)
      echo "  no .NET SDK download for $os $arch: install .NET $dotnet_sdk yourself"
      return 1
      ;;
  esac
  local scratch
  scratch="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "rm -rf '$scratch'" RETURN
  curl --proto '=https' --tlsv1.2 -sSfL -o "$scratch/releases.json" \
    "https://builds.dotnet.microsoft.com/dotnet/release-metadata/${dotnet_sdk%%.*}.0/releases.json"
  local meta url hash
  meta="$(python3 - "$scratch/releases.json" "$dotnet_sdk" "dotnet-sdk-$rid.tar.gz" << 'PY'
import json, sys
releases, version, name = json.load(open(sys.argv[1])), sys.argv[2], sys.argv[3]
for release in releases["releases"]:
    for sdk in release.get("sdks", []):
        if sdk["version"] == version:
            for f in sdk["files"]:
                if f["name"] == name:
                    print(f["url"], f["hash"])
                    sys.exit(0)
sys.exit(f"SDK {version} ({name}) is not in the release metadata")
PY
  )"
  read -r url hash <<< "$meta"
  echo "  downloading $url"
  curl --proto '=https' --tlsv1.2 -sSfL -o "$scratch/sdk.tar.gz" "$url"
  if command -v sha512sum > /dev/null; then
    echo "$hash  $scratch/sdk.tar.gz" | sha512sum --check --quiet
  else
    echo "$hash  $scratch/sdk.tar.gz" | shasum -a 512 --check --quiet
  fi
  mkdir -p "$HOME/.dotnet"
  tar -xzf "$scratch/sdk.tar.gz" -C "$HOME/.dotnet"
}

dotnet_() {
  say ".NET SDK $dotnet_sdk"
  if ! has_sdk; then
    if [[ $mode == check ]]; then
      miss "dotnet SDK $dotnet_sdk"
      return
    fi
    dotnet_sdk_install || {
      miss "dotnet SDK $dotnet_sdk"
      return
    }
  fi
  ok "dotnet $(cd "$root" && dotnet --version)"
  if [[ $mode == install ]]; then
    (cd "$root" && dotnet tool restore > /dev/null)
  fi
  if (cd "$root" && dotnet tool list --local 2> /dev/null | grep -q '^fable '); then ok "fable (dotnet tool)"; else miss "fable (dotnet tool restore)"; fi
}

# --- Node -----------------------------------------------------------------------------------

node_() {
  say "Node 22 (the WebAssembly and Fable smoke tests, markdownlint)"
  if command -v node > /dev/null && [[ $(node --version) == v2[2-9]* ]]; then
    ok "node $(node --version)"
  else
    echo "  optional: install Node 22 (https://nodejs.org) to run tools/wasm and tools/fable smoke tests"
  fi
}

# --- Extras for the other CI jobs -----------------------------------------------------------

cargo_tool() {
  local bin="$1" crate="$2" version="$3"
  if command -v "$bin" > /dev/null && "$bin" --version 2> /dev/null | grep -q " $version"; then
    ok "$bin $version"
  elif [[ $mode == check ]]; then
    miss "$bin $version (scripts/dev-setup.sh --full)"
  else
    cargo install --locked "$crate" --version "$version"
    ok "$bin $version"
  fi
}

extras() {
  say "Extra CI tools"
  cargo_tool wasm-bindgen wasm-bindgen-cli "$wasm_bindgen"
  cargo_tool cargo-about cargo-about "$cargo_about"
  echo "  the plugin guest's wasi-sdk is fetched by tools/plugin-guest/build.sh itself"
}

system_packages
rust
dotnet_
node_
[[ $full == 1 ]] && extras

echo
if [[ ${#missing[@]} != 0 ]]; then
  say "Missing: ${missing[*]}"
  [[ $mode == check ]] && echo "Run scripts/dev-setup.sh to install them."
  exit 1
fi

say "Ready. Add the tools to your shell:"
echo "  export PATH=\"\$HOME/.cargo/bin:\$HOME/.dotnet:\$HOME/.dotnet/tools:\$PATH\""
echo "Then:"
echo "  dotnet build FarmingRpgMaker.sln && dotnet test FarmingRpgMaker.sln --no-build"
echo "  cargo test --workspace"
echo "  dotnet run --project src/FarmingRpgMaker.App"
