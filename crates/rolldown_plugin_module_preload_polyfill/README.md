# rolldown_plugin_module_preload_polyfill

A plugin for [rolldown-vite](https://github.com/vitejs/rolldown-vite) that injects a `modulepreload` polyfill for legacy browsers, ported from [Vite's `modulePreloadPolyfillPlugin`](https://github.com/vitejs/vite/blob/main/packages/vite/src/node/plugins/modulePreloadPolyfill.ts#L34).

## 📦 What it does

This plugin resolves the special import:

```js
import 'vite/modulepreload-polyfill';
```

By default, it injects a polyfill for `rel="modulepreload"` into each entry module, ensuring compatibility with browsers that don't support `modulepreload` natively.

## 🚀 Usage

Add this plugin to your `plugins`:

```js
import { defineConfig } from 'rolldown';
import { modulePreloadPolyfillPlugin } from 'rolldown/experimental';

export default defineConfig({
  input: {
    entry: './index.ts',
  },
  plugins: [modulePreloadPolyfillPlugin()],
});
```

## 🧪 Compatibility

This plugin is originally designed for `Vite` but is more versatile.

Unlike `Vite`, which uses `__VITE_IS_MODERN__` to control the polyfill, this plugin automatically applies the polyfill only when `output.format` is `esm`, matching `Vite`’s behavior.

## 📄 License

### Polyfill License

The `module-preload-polyfill.js` polyfill code is based on
[es-module-shims](https://github.com/guybedford/es-module-shims) by `Guy Bedford`:

```
MIT License

Copyright (C) 2018-2021 Guy Bedford

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:
The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
```
