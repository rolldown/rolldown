# Coding Style

We recommend to follow these guidelines when writing code for rolldown. They aren't very strict rules since we want to be flexible and we understand that under certain circumstances some of them can be counterproductive. Just try to follow as many of them as possible:

## Rust

### General API Design

We tend to follow the suggestions of [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/). They are authored largely by the Rust library team, based on experiences building the Rust standard library and other crates in the Rust ecosystem.

We understand that there are cases that rules don't apply, but you should try to follow them as much as possible.

### Rule: File names should match the main struct, trait, enum or function name in that file

Examples:

- If a file implements a struct, like `Resolver` and `ResolverConfig`, the file should be named `resolver.rs`, because `Resolver` is the main struct implemented in that file.
- If a file contains only one struct, like `ResolverConfig`, the file should be named `resolver_config.rs` not `config.rs`.
- If a struct is complex enough to have its own folder, still prefer to put the struct into its own file with the same name as the struct. For example, move `bundler.rs` into `bundler/bundler.rs` instead of `bundler/mod.rs`.

Motivation:

When you're reasoning rolldown's codebase, you often think in terms of structs, functions, and traits. If file names correspond directly to struct names, it becomes much easier to locate the relevant code quickly. This is especially helpful in a large codebase like rolldown, where you might have many files and modules.

## Misc

### Adding tests

In general, we have two environments for running different purposes of tests. See [Testing](./testing.md) for more information.

We enquire that you should first considering adding tests in Rust side, because

- It has better debugging support without considering bridge between Rust and JavaScript.
- It has faster development cycle due to no need to compile the binding crate and run Node.js.

You could consider adding tests in Node.js with the following reasons:

- The test is about the behavior of the JavaScript API.
- The test is about the behavior of the `rolldown` package itself.
- E2E tests.
