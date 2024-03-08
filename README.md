<p align="center">
  <a href="https://rolldown.rs" target="_blank" rel="noopener noreferrer">
    <img width="180" src="https://rolldown.rs/rolldown-round.svg" alt="Vite logo">
  </a>
</p>

> 🚧 **Work in Progress**
>
> Rolldown is currently in active development and not usable for production yet.

# Rolldown

Rolldown is a JavaScript bundler written in Rust intended to serve as the future bundler used in Vite. It provides Rollup-compatible APIs and plugin interface, but will be more similar to esbuild in scope.

For more information, check out [rolldown.rs](https://rolldown.rs/about).

# Contributing

We would love to have more contributors involved! To get started, check out the [Contributing Guide](https://rolldown.rs/contrib-guide/).

# Credits

The Rolldown project is heavily inspired by:

- [Rollup](https://github.com/rollup/rollup), created by [Rich Harris](https://github.com/Rich-Harris) and maintained by [Lukas Taegert-Atkinson](https://github.com/lukastaegert).
- [esbuild](https://github.com/evanw/esbuild), created by [Evan Wallace](https://github.com/evanw).
- [@parcel/sourcemap](https://github.com/parcel-bundler/source-map).

And supported by:

- [napi-rs](https://github.com/napi-rs/napi-rs) for Node.js add-ons in Rust via Node-API.
- [oxc](https://github.com/oxc-project/oxc) for the underlying parser and resolver.

# Licenses

This project is licensed under the [MIT License](LICENSE).

This project also partially contains code derived or copied from the following projects:

- [rollup(MIT)](https://github.com/rollup/rollup/blob/680912e2ceb42c8d5e571e01c6ece0e4889aecbb/LICENSE-CORE.md)
- [esbuild(MIT)](https://github.com/evanw/esbuild/blob/0c8a0a901d9a6c7bbff9b4dd347c8a3f65f6c6dd/LICENSE.md)

Licenses of these projects are list in [THIRD-PARTY-LICENSE](/THIRD-PARTY-LICENSE)
