# Plugin sandbox guest

`farm_plugin_guest.wasm` is QuickJS compiled to WebAssembly (`wasm32-wasip1`), with fuel
metering added. `farm-plugins` embeds it and runs one instance per plugin in wasmi.

It is checked in, like the generated pixel art: building the engine never needs the C
toolchain. Rebuild it only when `crates/farm-plugin-guest`, `tools/plugin-guest/meter` or
`tools/plugin-guest/build.sh` changes:

```sh
tools/plugin-guest/build.sh
```

Then put the new hashes below. The `farm-plugins` tests fail until they match the file and its
inputs. CI rebuilds the guest from source and compares it byte for byte with this file
(`tools/plugin-guest/build.sh --check`), and runs `cargo fmt`, clippy and the meter's tests on
both crates (`--lint`).

## Recorded build

- sha256: `a9ded418fc937f3d7dc40a1f4b7ae0503c85f006e548679a71e05fdb01348f27`
- size: 736150 bytes
- inputs sha256: `716c10b47c93b3f03fc3487b77f23a0f5f69dc0584f03a0ed8d43eba9ff600f9`

The inputs hash covers the files of `crates/farm-plugin-guest` and `tools/plugin-guest/meter`
(without `target/`) and `tools/plugin-guest/build.sh`; the script prints it and
`crates/farm-plugins/tests/guest_wasm.rs` computes it the same way.

Two builds from the same inputs give the same bytes, wherever the repository is checked out
and whatever `RUSTFLAGS`, `CFLAGS` or `CC` the shell has: the script runs cargo with a cleared
environment and maps the C source paths to relative ones.

## Inputs

- Rust 1.94.1 (`rust-toolchain.toml`), target `wasm32-wasip1`. The Rust side links with
  Rust's own wasi-libc.
- `crates/farm-plugin-guest` and its `Cargo.lock`: `rquickjs-sys` 0.14.0 (quickjs-ng 0.16.2,
  with its pre-generated `wasm32-wasip1` bindings), `cc` 1.5.1.
- Release profile: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`,
  `strip = true`. Linker flags (`crates/farm-plugin-guest/.cargo/config.toml`):
  `--stack-first -zstack-size=1048576`.
- wasi-sdk 24.0 (`wasi-sdk-24.0-x86_64-linux.tar.gz`, sha256
  `c6c38aab56e5de88adf6c1ebc9c3ae8da72f88ec2b656fb024eda8d4167a0bc5`): its clang and
  sysroot compile the QuickJS C sources, with `-ffile-prefix-map=<guest crate>/=`.
- No `wasm-opt` (the script runs it only with `--wasm-opt`).
- `tools/plugin-guest/meter` and its `Cargo.lock` (`wasmparser` and `wasm-encoder`
  0.259.0) add the `fp_fuel` counter last.

## What the tests check in the binary

- The stack comes first: `__stack_pointer` starts at 1 MiB and every data segment starts at or
  above 1 MiB, so a stack overflow traps instead of overwriting data.
- Every `loop` checks the fuel counter at the top of each iteration, every function that calls
  another checks it on entry, and every `memory.copy`/`memory.fill`/`memory.init` is charged.

## Interface

ABI version 2 (`fp_abi_version`). Exports: `memory`, `fp_buffer`, `fp_prepare`, `fp_init`,
`fp_dispatch`, `fp_output_ptr`, `fp_output_len`, `fp_abi_version` and the `fp_fuel` global.
Imports: a few `wasi_snapshot_preview1` functions, which the host provides as inert stubs (no
files, no environment, a constant clock, deterministic random bytes), and `farm.host_call` /
`farm.host_answer`, the synchronous host calls behind `api.host` (the read API; version 1 had
no host calls). `crates/farm-plugin-guest/src/lib.rs` documents the ABI
and the sandbox rules; `tools/plugin-guest/meter/src/main.rs` documents the metering.
