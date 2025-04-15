# Maintenance Guide

A plugin for `rolldown-vite` that helps load `.wasm?init` files by converting them into proper `JavaScript` initialization code, ported from `Vite`'s [wasmHelperPlugin](https://github.com/vitejs/rolldown-vite/blob/fa334944/packages/vite/src/node/plugins/wasm.ts).

**This plugin is exclusive to `rolldown-vite` and is not recommended for external use.**

## 📦 What it does

This plugin enables the loading of `.wasm?init` imports by creating a wrapper JavaScript module that initializes the WebAssembly module at runtime.

It uses a virtual helper module (`\0vite/wasm-helper.js`) to handle the instantiation and setup of the WebAssembly module.

## ✅ Examples

```ts
import init from './module.wasm?init';
init().then(instance => {
  // use instance.exports...
});
```

Will be transformed to something like:

```ts
import initWasm from '\0vite/wasm-helper.js';
export default opts => initWasm(opts, 'assets/module-HASH.wasm');
```

## 🚀 Debug Usage

```js
import { defineConfig } from 'rolldown';
import { wasmHelperPlugin } from 'rolldown/experimental';

export default defineConfig({
  input: {
    entry: './main.ts',
  },
  plugins: [wasmHelperPlugin()],
});
```
