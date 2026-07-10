# Building orangu

`orangu` is a Rust project with three binaries: the interactive client
(`orangu`), an optional HTTP proxy that starts/stops llama.cpp on demand
(`orangu-coordinator`, see [doc/COORDINATOR.md](COORDINATOR.md)), and a
standalone CPU/GPU and GGUF file inventory tool (`orangu-gguf`, see
[doc/GGUF.md](GGUF.md)).

## Prerequisites

- Rust toolchain with `cargo`
- A running llama.cpp server exposing an OpenAI-compatible API

## Build

```sh
cargo build
```

For an optimized build:

```sh
cargo build --release
```

## Test

```sh
cargo test
```

## Manual generation

The project includes a pandoc-based manual layout under `doc/manual/en`.

To build the manual:

```sh
./doc/build_manual.sh
```

The script writes HTML and PDF output to `target/doc/`.

## Example run

```sh
cargo run --bin orangu -- --config ./doc/etc/orangu.conf
```

```sh
cargo run --bin orangu-coordinator -- --config ./doc/etc/orangu-coordinator.conf
```

```sh
cargo run --bin orangu-gguf -- system
cargo run --bin orangu-gguf -- --config ./doc/etc/orangu-gguf.conf list
```
