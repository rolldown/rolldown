# Building and running

Make sure you have gone through the [setup process](./setup-the-project.md) before continuing.

## What is `just`?

`just` is a command runner for the `rolldown` repository. It could build, test, and lint the project with a single command.

### Usage

You could get a list of available commands by running the command `just` only.

### Important Commands

- `just roll` - Build rolldown from scratch and run all the tests and checks.
- `just test` - Runs all tests.
- `just lint` - Lint the codebase.
- `just fmt` - Fix formatting issues.

> Most of commands will run both Rust and Node.js scripts. To only target one, append `-rust` or `-node` to the just command. For example, `just lint-rust` or `just check-node`.

::: tip
`just roll` would be the most used command in your development workflow. It will help you, without any thinking, to check if everything is working correctly for any changes you made.

It will help you catch errors locally rather than pushing your changes to GitHub and waiting for the CI.

- `just roll-rust` - Run only Rust checks.
- `just roll-node` - Run only Node.js checks.
- `just roll-repo` - Checks for non-code related issues, like file na

:::

## Building

Rolldown is built on Rust and Node.js, so a building process includes building Rust crates, Node.js packages and the glue part that binds them together. The glue part is also a Node.js package, but building it will also trigger building the Rust crates.

Luckily, NAPI-RS has encapsulated the process of building the glue part, we don't need to worry about the details.

## `rolldown`

To build the `rolldown` package, there are two commands:

- `just build`/`just build native`
- `just build native release` (**important if running benchmarks**)

They will automatically build the Rust crates and the Node.js package. So no matter what changes you made, you can always run these commands to build the latest `rolldown` package.

### WASI

Rolldown supports WASI by considering is as a special platform. So we still use the `rolldown` package to distribute the WASI version of Rolldown.

To build the WASI version, you can run the following command:

- `just build wasi`
- `just build wasi release` (**important if running benchmarks**)

Building the WASI version will remove the native version of Rolldown. We designed the local build process on purpose that is you either build the native version or the WASI version. You can't mix them together, though NAPI-RS supports it.

## Running

The `rolldown` package is linked to `node_modules` via pnpm workspace automatically.

Once you have built the `rolldown` package, you can run it with the following command:

```sh
pnpm rolldown
```
