# Coding Style

We recommend to follow these guidelines when writing code for rolldown. They aren't very strict rules since we want to be flexible and we understand that under certain circumstances some of them can be counterproductive. Just try to follow as many of them as possible:

## Adding tests

In generate, we have two environments for running different purposes of tests. See [Testing](./testing.md) for more information.

We enquire that you should first considering adding tests in rust side, because

- It has better debugging support without considering bridge between rust and js.
- It has faster development cycle due to no need to compile the binding crate and run Node.js.

You could consider adding tests in Node.js with the following reasons:

- The test is about the behavior of the JavaScript API.
- The test is about the behavior of the `rolldown` package itself.
- e2e tests.

## Misc

### Concat strings

We prefer using [`concat-string`](https://crates.io/crates/concat-string) crate to concatenate strings. There are some advantages:

- It is more efficient than the std's `format` macro. You could see the benchmark [here](https://github.com/hoodie/concatenation_benchmarks-rs).
- Especially when you are manipulating JavaScript code, it doesn't need to worried about escaping characters `{` and `}`.
