# Project Setup

## Prerequisites

- Rust >= 1.75
- Node.js >= v18
- Yarn >= 4
- Git

## Setup

On your first checkout of the repository, you'll need to install required tools and dependencies.

:::tip
If you are using [proto](https://moonrepo.dev/proto), you can run `proto use` in the repository root to install Rust, Node.js, and Yarn. For other tools, or if not using proto, continue reading.
:::

### Rust

Rolldown is built on Rust and requires `rustup` and `cargo` to exist in your environment. You can
[install Rust from the official website](https://www.rust-lang.org/tools/install).

We also require `just` and `cargo-binstall`. You can install these with:

```shell
cargo install cargo-binstall
cargo binstall just
```

Once installed, run the following to install secondary tools.

```shell
just init-rust  # Run this command in the directory where the `rolldown` project located
```

### Node.js

Rolldown is a npm package built with [NAPI-RS](https://napi.rs/) and is published to the npm registry, and as such requires Node.js and Yarn (for dependency management).

We recommend installing Node.js with a version manager, like [nvm](https://github.com/nvm-sh/nvm) or [fnm](https://github.com/Schniz/fnm). Make sure to install and use Node.js version 18+, which is the minimum requirement for this project. You can skip this step if you are already using a Node.js version manager of your choice and on a Node.js version that meets the requirement.

We recommend enabling Yarn via [corepack](https://nodejs.org/api/corepack.html), so the correct version of Yarn can be automatically used when working in this project:

```shell
corepack enable
```

Once both Node.js and Yarn are configured, run the following to install dependencies.

```shell
just init-node
```

## High Level Workflow

The following commands are available and should be used in your standard development workflow.

- `just init` - Install required tools and dependencies.
- `just check` - Runs the type checker.
- `just lint` - Lints code.
- `just fmt` - Formats code.
- `just test` - Runs tests. Also see [Testing](./test.md).
- `just ready` - Run everything!

> Every command will run both Rust and Node.js scripts. To only target one, append `-rust` or `-node` to the just command. For example, `just lint-rust` or `just fmt-node`.
