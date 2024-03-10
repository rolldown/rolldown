# Project Setup

## Prerequisites

- Rust >= 1.75
- Node.js >= 18.18.0
- Yarn >= 4
- Git

## Setup

On your first checkout of the repository, you'll need to install required tools and dependencies.

:::tip
If you are using [proto](https://moonrepo.dev/proto), you can run `proto use` in the repository root to install Rust, Node.js, and Yarn. For other tools, or if not using proto, continue reading.
:::

We have made initializing the project automatic with the `just` command. However, you still need to install some meta tools and dependencies before you can use the command.

### Rust

Rolldown is built on Rust and requires `rustup` and `cargo` to exist in your environment. You can
[install Rust from the official website](https://www.rust-lang.org/tools/install).

We also require `just` and `cargo-binstall`. You can install these with:

```shell
cargo install cargo-binstall
cargo binstall just
```

### Node.js

Rolldown is a npm package built with [NAPI-RS](https://napi.rs/) and is published to the npm registry, and as such requires Node.js and Yarn (for dependency management).

We recommend installing Node.js with a version manager, like [nvm](https://github.com/nvm-sh/nvm) or [fnm](https://github.com/Schniz/fnm). Make sure to install and use Node.js version 18.18.0+, which is the minimum requirement for this project. You can skip this step if you are already using a Node.js version manager of your choice and on a Node.js version that meets the requirement.

We recommend enabling Yarn via [corepack](https://nodejs.org/api/corepack.html), so the correct version of Yarn can be automatically used when working in this project:

```shell
corepack enable
```

### Init

Once Rust, Node.js and Yarn are installed, run the following command to install all required dependencies:

```shell
just init
```

After initialization, everything should be set up and ready to go. You could run

```shell
just smoke
```

to verify that everything is set up correctly.

::: tip
`just smoke` command almost run all ci checks locally. It's useful to run this before pushing your changes. It also has two variants `just smoke-rust` and `just smoke-node` to run only Rust or Node.js checks.
:::

## High Level Workflow

The following commands are available and should be used in your standard development workflow.

- `just init` - Install required tools and dependencies.
- `just smoke` - Runs various kinds of checks to ensure the project is in a good state.
- `just lint` - Lint the codebase.
- `just fmt` - Fix formatting issues.
- `just check` - Run the type checker.
- `just test` - Runs tests. Also see [Testing](./test.md).

> Every command will run both Rust and Node.js scripts. To only target one, append `-rust` or `-node` to the just command. For example, `just lint-rust` or `just fmt-node`.

::: tip
You could run the command `just` only and it will show you all available commands.
:::
