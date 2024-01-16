# Contributing

Contributions are always welcome, no matter how large or small!

## Prerequisites

- Rust >= 1.75
- Node.js >= v18
- Yarn >= 4
- Git

## Setup

On your first checkout of the repository, you'll need to install required tools and dependencies.

> If you are using [proto](https://moonrepo.dev/proto), you can run `proto use` in the repository root to install Rust, Node.js, and Yarn. For other tools, or if not using proto, continue reading.

### Rust

Rolldown is built on Rust and requires `rustup` and `cargo` to exist in your environment. You can
[install Rust from the official website](https://www.rust-lang.org/tools/install), or with [proto](#setup).

We also require `just` and `cargo-binstall`. You can install these with:

```shell
cargo install cargo-binstall
cargo binstall just
```

Once installed, run the following to install secondary tools.

```shell
just init-rust
```

### Node.js

Rolldown is an npm package built with [NAPI-RS](https://napi.rs/) and is published to the npm registry, and as such requires Node.js and Yarn (for dependency management).

Begin by installing Node.js with a version manager, like [proto](https://moonrepo.dev/proto) or [nvm](https://github.com/nvm-sh/nvm).

```shell
proto install node
# or
nvm install node
```

Yarn can then be installed with proto, corepack, or npm.

```shell
proto install yarn
# or
corepack enable
# or
npm install -g yarn
```

Once installed, run the following to install dependencies.

```shell
just init-node
```

## Workflow

The following commands are available and should be used in your standard development workflow.

- `just init` - Install required tools and dependencies.
- `just check` - Runs the typechecker.
- `just lint` - Lints code.
- `just fmt` - Formats code.

> Every command will run both Rust and Node.js scripts. To only target one, append `-rust` or `-node` to the just command. For example, `just lint-rust` or `just fmt-node`.

## How to

### Open development

All development happens directly on GitHub. Both core team members and external contributors (via forks)
send pull requests which go through the same review process.

### Branch organization

Submit all pull requests directly to the `main` branch. We only use separate branches for upcoming
releases / breaking changes, otherwise, everything points to master.

Code that lands in main must be compatible with the latest stable release. It may contain
additional features, but no breaking changes. We should be able to release a new minor version from
the tip of main at any time.

> Developers use (or may use) [Graphite](https://graphite.dev/) for better branch management.

### Reporting a bug

Please report bugs to GitHub
only after you have previously searched for the issue and found no results. Be sure to be as
descriptive as possible and to include all applicable labels.

The best way to get your bug fixed is to provide a reduced test case. Please provide a public
repository with a runnable example, or a usable code snippet.

### Requesting new functionality

Before requesting new functionality, view [open issues](https://github.com/rolldown/rolldown/issues) as
your request may already exist. If it does not exist, submit an issue with the title prefixed with `[request]`.
Be sure to be as descriptive as possible and to include all applicable labels.

### Submitting a pull request

We accept pull requests for all bugs, fixes, improvements, and new features. Before submitting a
pull request, be sure your build passes locally using the development workflow above.
