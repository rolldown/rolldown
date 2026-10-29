# Maintenance Guide

A plugin for `vite` that handles loading files with `query` or `hash` fragments, ported from `Vite`'s [loadFallbackPlugin](https://github.com/vitejs/rolldown-vite/blob/fa334944/packages/vite/src/node/plugins/loadFallback.ts).

> [!NOTE]
> This plugin is exclusive to `vite`; external use is not recommended.
> Its API may change between minor versions of `rolldown`, but
> stays compatible within the same minor version.

## 📦 What it does

This plugin provides a fallback mechanism for module IDs with query (`?`) or hash (`#`) fragments. It strips the `query/hash`, loads the file from the filesystem, and adds the stripped path to the watch list for hot reloading.

## 🚀 Debug Usage

```js
import { defineConfig } from 'rolldown';
import { viteLoadFallbackPlugin } from 'rolldown/experimental';

export default defineConfig({
  input: {
    entry: './main.ts',
  },
  plugins: [viteLoadFallbackPlugin()],
});
```

## 🧪 Porting Differences

In Vite’s plugin, if reading the cleaned path fails, it falls back to reading the original full ID (including the `query/hash`).

In contrast, Rolldown’s plugin only attempts to read the cleaned path and does not retry the full ID, leaving such cases to be handled by the rolldown core loading logic.
