# Maintenance Guide

A plugin for `vite` that generates a `manifest.json` mapping original filenames to emitted assets/chunks, ported from `Vite`'s [manifestPlugin](https://github.com/vitejs/rolldown-vite/blob/fa33494/packages/vite/src/node/plugins/manifest.ts).

> [!NOTE]
> This plugin is exclusive to `vite`; external use is not recommended.
> Its API may change between minor versions of `rolldown`, but
> stays compatible within the same minor version.

## 📦 What it does

This plugin collects all emitted chunks and assets, associates them with their original names,
and outputs a manifest JSON. Useful for server-side rendering or asset injection.

## 🚀 Debug Usage

```js
import { defineConfig } from 'rolldown';
import { viteManifestPlugin } from 'rolldown/experimental';

export default defineConfig({
  input: {
    entry: './main.ts',
  },
  plugins: [
    viteManifestPlugin({
      root: path.resolve(import.meta.dirname),
      outPath: path.resolve(import.meta.dirname, 'dist/manifest.json'),
    }),
  ],
});
```

## ⚙️ Options

| Option    | Type     | Description                             |
| --------- | -------- | --------------------------------------- |
| `root`    | `string` | Project root directory                  |
| `outPath` | `string` | Where to write the manifest output file |
