<p align="center">
  <a href="https://rolldown.rs" target="_blank" rel="noopener noreferrer">
    <img width="180" src="https://rolldown.rs/rolldown-round.svg" alt="Rolldown logo">
  </a>
</p>

<div align="center">

[![MIT licensed][badge-license]][url-license]
[![NPM version][badge-npm-version]][url-npm]
[![CodSpeed Badge](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://codspeed.io/rolldown/rolldown)
[![Discord chat][badge-discord]][discord-url]
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/rolldown/rolldown)

</div>

<div align="center">

[![NPM Unpacked Size (with version)](https://img.shields.io/npm/unpacked-size/rolldown/latest?label=npm)][url-npm]
[![NPM Unpacked Size darwin-arm64](https://img.shields.io/npm/unpacked-size/%40rolldown%2Fbinding-darwin-arm64/latest?label=darwin-arm64)](https://www.npmjs.com/package/@rolldown/binding-darwin-arm64)
[![NPM Unpacked Size darwin-x64](https://img.shields.io/npm/unpacked-size/%40rolldown%2Fbinding-darwin-x64/latest?label=darwin-x64)](https://www.npmjs.com/package/@rolldown/binding-darwin-x64)
[![NPM Unpacked Size linux-x64-gnu](https://img.shields.io/npm/unpacked-size/%40rolldown%2Fbinding-linux-x64-gnu/latest?label=linux-x64-gnu)](https://www.npmjs.com/package/@rolldown/binding-linux-x64-gnu)
[![NPM Unpacked Size win32-x64](https://img.shields.io/npm/unpacked-size/%40rolldown%2Fbinding-win32-x64-msvc/latest?label=win32-x64)](https://www.npmjs.com/package/@rolldown/binding-win32-x64-msvc)
[![NPM Unpacked Size wasm32-wasi](https://img.shields.io/npm/unpacked-size/%40rolldown%2Fbinding-wasm32-wasi/latest?label=wasm32-wasi)](https://www.npmjs.com/package/@rolldown/binding-wasm32-wasi)

</div>

<div align="center">

[![pkg.pr.new](https://pkg.pr.new/badge/pkg.pr.new/pkg.pr.new?style=flat&color=000&logoSize=auto)](https://pkg.pr.new/~/rolldown/rolldown)

</div>

<div align="center">

[![rolldown-starter-stackblitz](https://developer.stackblitz.com/img/open_in_stackblitz.svg)](https://stackblitz.com/fork/github/rolldown/rolldown-starter-stackblitz)

</div>

> 🚧 **Beta Software**
>
> Rolldown is currently in beta status. While it can already handle most production use cases, there may still be bugs and rough edges. Most notably, the built-in minification feature is still in alpha status.

# Rolldown

Rolldown is a JavaScript/TypeScript bundler written in Rust intended to serve as the future bundler used in [Vite](https://vitejs.dev/). It provides Rollup-compatible APIs and plugin interface, but will be more similar to esbuild in scope.

For more information, please check out the documentation at [rolldown.rs](https://rolldown.rs/about).

## VoidZero Inc.

Rolldown is a project of [VoidZero](https://voidzero.dev/), see our announcement [Announcing VoidZero - Next Generation Toolchain for JavaScript](https://voidzero.dev/posts/announcing-voidzero-inc).

If you have requirements for JavaScript tools at scale, please [get in touch](https://forms.gle/WQgjyzYJpwurpxWKA)!

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

[badge-discord]: https://img.shields.io/discord/1079625926024900739?logo=discord&label=Discord
[discord-url]: https://chat.rolldown.rs
[badge-license]: https://img.shields.io/badge/license-MIT-blue.svg
[url-license]: https://github.com/rolldown/rolldown/blob/main/LICENSE
[badge-npm-version]: https://img.shields.io/npm/v/rolldown/latest?color=brightgreen
[url-npm]: https://www.npmjs.com/package/rolldown/v/latest
[badge-binary-size-windows]: [https://img.shields.io/npm/unpacked-size/%40rolldown%2Fbinding-win32-x64-msvc/latest]
[badge-binary-size-macos]: [https://img.shields.io/npm/unpacked-size/%40rolldown%2Fbinding-darwin-arm64/latest]
