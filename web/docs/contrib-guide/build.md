# Building

Rolldown is built on Rust and Node.js, so and building process includes building Rust crates, Node.js packages and the glue part that binds them together. The glue part is also a Node.js package, but building it will also trigger building the Rust crates.

Luckily, NAPI-RS has encapsulated the process of building the glue part, we don't need to worry about the details.

## `rolldown`

To build the `rolldown` package, there are two commands:

- `yarn build`
- `yarn build:release` (**important if running benchmarks**)

They will automatically build the Rust crates and the Node.js package. So no matter what changes you made, you can always run these commands to build the latest `rolldown` package.
