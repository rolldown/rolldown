<script setup lang="ts">
  import { data } from '../data-loading/node-version.data.js'
</script>

# Setup the project

## Prerequisites

Only a few tools are required to build and run Rolldown. You'll need:

- Install Rust via [rustup](https://www.rust-lang.org/tools/install)
- Install `just`

You could install `just` quickly by running the following command or by following the official [guide](https://github.com/casey/just?tab=readme-ov-file#installation):

::: code-group

```sh [Npm]
npm install --global just-install
```

```sh [Pnpm]
pnpm --global add just-install
```

```sh [Yarn]
yarn global add just-install
```

```sh [Homebrew]
brew install just
```

```sh [Cargo]
cargo install just
```

:::

- Install `cmake`

You could install just quickly by running the following command or by following the official [Download](https://cmake.org/download/):

::: code-group

```sh [Mac]
brew install cmake
```

```sh [Windows]
choco install cmake
```

:::

- Install Node.js >= {{ data.nodeVersion }} / 21.2.0

## `just setup`

On your first checkout of the repository, all you need to do is run `just setup` in the repository root.

If you are seeing `✅✅✅ Setup complete!` at the end, that means you have everything you need to build and run rolldown.

You could run `just roll` to verify if everything is working correctly.

::: tip

- `just roll` might take a while to run, since it will build rolldown from scratch and run all the tests.
- If you want to know what `just setup` does under the hood, you can check the [`justfile`](https://github.com/rolldown/rolldown/blob/main/justfile) in the repository root.

:::

Now, you could move to next chapter [Building and Running](./building-and-running.md). Continue reading if you want to have a in-depth understanding of the setup process.

## In Depth

This section will go into more detail about the installed tools and dependencies required to build and run Rolldown.

### Setup Rust

Rolldown is built on Rust and requires `rustup` and `cargo` to exist in your environment. You can
[install Rust from the official website](https://www.rust-lang.org/tools/install).

### Setup Node.js

Rolldown is a npm package built with [NAPI-RS](https://napi.rs/) and is published to the npm registry, and as such requires Node.js and pnpm (for dependency management).

We recommend installing Node.js with a version manager, like [nvm](https://github.com/nvm-sh/nvm) or [fnm](https://github.com/Schniz/fnm). Make sure to install and use Node.js version {{ data.nodeVersion }}+, which is the minimum requirement for this project. You can skip this step if you are already using a Node.js version manager of your choice and on a Node.js version that meets the requirement.

#### Setup pnpm

We recommend enabling pnpm via [corepack](https://nodejs.org/api/corepack.html), so the correct version of pnpm can be automatically used when working in this project:

```shell
corepack enable
```

to verify that everything is setup correctly.
