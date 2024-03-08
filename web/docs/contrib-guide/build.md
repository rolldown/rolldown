# Building Bindings

For the NAPI-RS based Node packages to work, and for their tests and benchmarks to run, they must be built first. This is done by running `yarn build` in the root directory. This will spin up a process that builds the Node/WASM binding crates (with Cargo), and then builds the rolldown npm package. The `yarn build` script is also smart enough to only re-build if it detects changes since the last time it was run.

`yarn build` accepts two flags:

- `--no-wasm`
- `--release` (**important if running benchmarks**)

In addition, the `yarn watch` script can be used to watch the file system for changes, and re-build the bindings and npm package when changes are detected (using the same process as `yarn build`). This is useful for development when you're constantly making changes and re-running tests.
