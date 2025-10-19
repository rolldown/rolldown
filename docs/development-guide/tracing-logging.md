# Tracing/Logging

Rolldown's codebase has a lot of [`tracing::debug!`] (or `tracing::trace!`) calls, which print out logging information at many points. These are very useful to at least narrow down the location of a bug if not to find it entirely, or just to orient yourself as to why the compiler is doing a particular thing.

[`tracing::debug!`]: https://docs.rs/tracing/0.1/tracing/macro.debug.html

To see the logs, you need to set the `RD_LOG` environment variable to your log filter. The full syntax of the log filters can be found in the [rustdoc of `tracing-subscriber`](https://docs.rs/tracing-subscriber/0.2.24/tracing_subscriber/filter/struct.EnvFilter.html#directives).

## Usages

```
RD_LOG=debug [executing rolldown]
RD_LOG=debug RD_LOG_OUTPUT=chrome-json [executing rolldown]
```

## Add logging

It's fine to add `tracing::debug!` or `tracing::trace!` calls in your PRs. However, to avoid noise in the logs, you should be careful about choosing `tracing::debug!` or `tracing::trace!`.

There are some rules that help you to choose right logging level:

- If you don't know what level to choose, use `tracing::trace!`.
- If the log message would only be printed once during the bundling, use `tracing::debug!`.
- If the log message would only be printed once but the size of content is related to the scale of input during the bundling, use `tracing::trace!`.
- If the log message would be printed multiple but limited times during the bundling, use `tracing::debug!`.
- If the log message would be printed multiple times due to the scale of the input, use `tracing::trace!`.

These rules also apply to the `#[tracing::instrument]` attribute.

- If the function is called only once during the bundling, use `#[tracing::instrument(level = "debug", skip_all)]`.
- If the function is called multiple times due to the scale of the input, use `#[tracing::instrument(level = "trace", skip_all]`.

::: info
What information should be traced could be opinionated, so the reviewer will decide whether to let you leave tracing statements in or whether to ask you to remove them before merging.
:::

## Function level filters

Lots of functions in rolldown are annotated with

```
#[instrument(level = "debug", skip(self))]
fn foo(&self, bar: Type) {}

#[instrument(level = "debug", skip_all)]
fn baz(&self, bar: Type) {}
```

which allows you to use

```
RUSTC_LOG=[foo]
```

to do the following all at once

- log all function calls to `foo`
- log the arguments (except for those in the `skip` list)
- log everything (from anywhere else in the compiler) until the function returns

Notices:

We generally recommend using `skip_all` unless you have a good reason to use logging for the arguments.

## Trace Module Resolution

Rolldown uses [oxc-resolver](https://github.com/oxc-project/oxc-resolver), which exposes trace information for debugging purposes.

```bash
RD_LOG='oxc_resolver' rolldown
```

This emits trace information for the `oxc_resolver::resolve` function, e.g.

```
2024-06-11T07:12:20.003537Z DEBUG oxc_resolver: options: ResolveOptions { ... }, path: "...", specifier: "...", ret: "..."
    at /path/to/oxc_resolver-1.8.1/src/lib.rs:212
    in oxc_resolver::resolve with path: "...", specifier: "..."
```

The input values are `options`, `path` and `specifier`, the returned value is `ret`.
