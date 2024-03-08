# Benchmarking

## Setup

Before running the benchmarks, set up the necessary fixtures with:

```shell
# in project root
just setup-bench
```

## Benchmarking in Rust

```shell
# in project root
just bench
```

## Benchmarking in Node.js

Make sure to build the Node.js bindings in release mode:

```shell
# in project root
yarn build --release
```

Then, in `packages/bench`:

```shell
# in packages/bench
yarn bench
```
