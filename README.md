<p align="center">
  <a href="https://rolldown.rs" target="_blank" rel="noopener noreferrer">
    <img width="180" src="https://rolldown.rs/rolldown-round.svg" alt="Rolldown logo">
  </a>
</p>

<div align="center">

[![MIT licensed][license-badge]][license-url]
[![Build Status][ci-badge]][ci-url]
[![Code Coverage][code-coverage-badge]][code-coverage-url]
[![CodSpeed Badge](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://codspeed.io/rolldown/rolldown)
[![Discord chat][discord-badge]][discord-url]

</div>

> 🚧 **Work in Progress**
>
> Rolldown is currently in active development and not usable for production yet.

# Rolldown

Rolldown is a JavaScript bundler written in Rust intended to serve as the future bundler used in [Vite](https://vitejs.dev/). It provides Rollup-compatible APIs and plugin interface, but will be more similar to esbuild in scope.

For more information, please check out the documentation at [rolldown.rs](https://rolldown.rs/about).

## Contributing

We would love to have more contributors involved!

To get started, please read our [Contributing Guide](https://rolldown.rs/contrib-guide/).

## Credits

The Rolldown project is heavily inspired by:

- [Rollup](https://github.com/rollup/rollup), created by [Rich Harris](https://github.com/Rich-Harris) and maintained by [Lukas Taegert-Atkinson](https://github.com/lukastaegert).
- [esbuild](https://github.com/evanw/esbuild), created by [Evan Wallace](https://github.com/evanw).

And supported by:

- [napi-rs](https://github.com/napi-rs/napi-rs) for Node.js add-ons in Rust via Node-API.
- [oxc](https://github.com/oxc-project/oxc) for the underlying parser, resolver, and sourcemap support.

## Licenses

This project is licensed under the [MIT License](LICENSE).

This project also partially contains code derived or copied from the following projects:

- [rollup(MIT)](https://github.com/rollup/rollup/blob/680912e2ceb42c8d5e571e01c6ece0e4889aecbb/LICENSE-CORE.md)
- [esbuild(MIT)](https://github.com/evanw/esbuild/blob/0c8a0a901d9a6c7bbff9b4dd347c8a3f65f6c6dd/LICENSE.md)

Licenses of these projects are listed in [THIRD-PARTY-LICENSE](/THIRD-PARTY-LICENSE)

[discord-badge]: https://img.shields.io/discord/1079625926024900739?logo=discord&label=Discord
[discord-url]: https://chat.rolldown.rs
[license-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[license-url]: https://github.com/rolldown/rolldown/blob/main/LICENSE
[ci-badge]: https://github.com/rolldown/rolldown/actions/workflows/ci.yml/badge.svg?event=push&branch=main
[ci-url]: https://github.com/rolldown/rolldown/actions/workflows/ci.yml?query=event%3Apush+branch%3Amain
[npm-badge]: https://img.shields.io/npm/v/rolldown/latest?color=brightgreen
[npm-url]: https://www.npmjs.com/package/rolldown/v/latest
[code-coverage-badge]: https://codecov.io/github/rolldown/rolldown/branch/main/graph/badge.svg
[code-coverage-url]: https://codecov.io/gh/rolldown/rolldown
