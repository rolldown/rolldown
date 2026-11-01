<div align="center">

[![Crates.io][crates-badge]][crates-url]
[![Docs.rs][docs-badge]][docs-url]
[![BSD-3-Clause licensed][license-badge]][license-url]
[![Build Status][ci-badge]][ci-url]
[![Discord chat][discord-badge]][discord-url]

<!-- [![Code Coverage][code-coverage-badge]][code-coverage-url] -->
<!-- [![CodSpeed Badge][codspeed-badge]][codspeed-url] -->

</div>

# oxc-sourcemap

Forked version of [rust-sourcemap](https://github.com/getsentry/rust-sourcemap), modified for Oxc.

## Benchmarking

Run benchmarks with:

```bash
cargo bench --bench simple
```

`benches/simple.rs` contains grouped benchmarks for parse, serialize, lookup table generation, and
concat workflows. Fixture-driven benchmarks load maps from `tests/fixtures/perf/*.map`.

[discord-badge]: https://img.shields.io/discord/1079625926024900739?logo=discord&label=Discord
[discord-url]: https://discord.gg/9uXCAwqQZW
[license-badge]: https://img.shields.io/badge/license-BSD--3--Clause-blue.svg
[license-url]: https://github.com/oxc-project/oxc-sourcemap/blob/main/LICENSE
[ci-badge]: https://github.com/oxc-project/oxc-sourcemap/actions/workflows/ci.yml/badge.svg?event=push&branch=main
[ci-url]: https://github.com/oxc-project/oxc-sourcemap/actions/workflows/ci.yml?query=event%3Apush+branch%3Amain
[code-coverage-badge]: https://codecov.io/github/oxc-project/oxc-sourcemap/branch/main/graph/badge.svg
[code-coverage-url]: https://codecov.io/gh/oxc-project/oxc-sourcemap
[codspeed-badge]: https://img.shields.io/endpoint?url=https://codspeed.io/badge.json
[codspeed-url]: https://codspeed.io/oxc-project/oxc-sourcemap
[crates-badge]: https://img.shields.io/crates/d/oxc_sourcemap?label=crates.io
[crates-url]: https://crates.io/crates/oxc_sourcemap
[docs-badge]: https://img.shields.io/docsrs/oxc_sourcemap
[docs-url]: https://docs.rs/oxc_sourcemap

# [Sponsored By](https://oxc.rs/sponsor)

<p align="center">
  <a href="https://oxc.rs/sponsor">
    <img src="https://raw.githubusercontent.com/oxc-project/sponsors/main/sponsors.svg" alt="Our sponsors" />
  </a>
</p>
