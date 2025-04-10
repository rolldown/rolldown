# Maintenance Guide

A plugin for `rolldown-vite` that provides a fallback error message for `.wasm` files, ported from `Vite`'s [wasmPlugin](https://github.com/vitejs/rolldown-vite/blob/fa33494473d41956fa16ae441d9a4be98bd192d0/packages/vite/src/node/plugins/wasm.ts#L78-L94).

**This plugin is exclusive to `rolldown-vite` and is not recommended for external use.**

## 📦 What it does

This plugin intercepts all imports ending in `.wasm` and throws an informative error.

Since native ESM integration for WebAssembly is not yet supported, this plugin reminds users to use community plugins or explicit suffixes such as `?init` or `?url`.

## 🚀 Debug Usage

```js
import { defineConfig } from 'rolldown';
import { wasmFallbackPlugin } from 'rolldown/experimental';

export default defineConfig({
  input: {
    entry: './main.ts',
  },
  plugins: [wasmFallbackPlugin()],
});
```
