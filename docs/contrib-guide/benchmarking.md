# Benchmarking

## Setup

Before running the benchmarks, set up the necessary fixtures with:

```shell
# in project root
just setup-bench
```

## Benchmarking in Rust

`bench-rust` will build the Rust code automatically, so you don't need to build yourself.

```shell
# in project root
just bench-rust
```

## Benchmarking in Node.js

Make sure to build the Node.js bindings in release mode:

```shell
just build-rolldown-release
```

Then run

```sh
just bench-node
```
