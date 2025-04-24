# Maintenance Guide

A plugin for `rolldown-vite` that generates a `manifest.json` mapping original filenames to emitted assets/chunks, ported from `Vite`'s [manifestPlugin](https://github.com/vitejs/rolldown-vite/blob/fa33494/packages/vite/src/node/plugins/manifest.ts).

**This plugin is exclusive to `rolldown-vite` and is not recommended for external use.**

## 📦 What it does

This plugin collects all emitted chunks and assets, associates them with their original names,
and outputs a manifest JSON. Useful for server-side rendering or asset injection.

## 🚀 Debug Usage

```js
import { defineConfig } from 'rolldown';
import { manifestPlugin } from 'rolldown/experimental';

export default defineConfig({
  input: {
    entry: './main.ts',
  },
  plugins: [manifestPlugin({
    root: path.resolve(import.meta.dirname),
    outPath: path.resolve(import.meta.dirname, 'dist/manifest.json'),
  })],
});
```

## ⚙️ Options

| Option    | Type     | Description                             |
| --------- | -------- | --------------------------------------- |
| `root`    | `string` | Project root directory                  |
| `outPath` | `string` | Where to write the manifest output file |
