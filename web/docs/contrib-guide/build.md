# Building

Rolldown is built on Rust and Node.js, so and building process includes building Rust crates, Node.js packages and the glue part that binds them together. The glue part is also a Node.js package, but building it will also trigger building the Rust crates.

Luckily, NAPI-RS has encapsulated the process of building the glue part, we don't need to worry about the details.

Typically, rolldown has two main packages:

- `rolldown`, which is the final npm package that users will install directly.
- `@rolldown/node-binding`, which is the glue part that binds Rust and Node.js together.

## Incremental Build

For the NAPI-RS based Node packages to work, and for their tests and benchmarks to run, they must be built first. This is done by running `yarn build` in the root directory. This will spin up a process that builds the Node/WASM binding crates (with Cargo), and then builds the rolldown npm package.

The `yarn build` script is also smart enough to only re-build if it detects changes since the last time it was run.

`yarn build` accepts two flags:

- `--no-wasm`
- `--release` (**important if running benchmarks**)

In addition, the `yarn watch` script can be used to watch the file system for changes, and re-build the bindings and npm package when changes are detected (using the same process as `yarn build`). This is useful for development when you're constantly making changes and re-running tests.

## Fresh Build

If you want to force a fresh build on `rolldown`, you can run `yarn build:node`. It will topologically run the `build` command in `rolldown` and `@rolldown/node-binding` package.

To build some specific package only:

For `rolldown`, you could run:

- `yarn workspace rolldown build`.

For `@rolldown/node-binding`, you could run:

- `yarn workspace @rolldown/node-binding build`
- `yarn workspace @rolldown/node-binding build:release` (**important if running benchmarks**)

:::warning

Due to the boundary between Rust and Node.js, once you changed the Rust code, you need to rebuild the `@rolldown/node-binding` package to see the changes while executing in Node.js.

:::
