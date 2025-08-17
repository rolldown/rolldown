# Maintenance Guide

A plugin for `rolldown-vite` that enables loading `data:` URIs as modules, ported from `Vite`'s [dataUriPlugin](https://github.com/vitejs/rolldown-vite/blob/03e6286b52f4c1cf9c3ede2366bff685549b3860/packages/vite/src/node/plugins/dataUri.ts).

**This plugin is exclusive to `rolldown-vite` and is not recommended for external use.**

## 📦 What it does

This plugin enables native support for `data:` URLs in JavaScript, CSS, and JSON contexts by resolving and loading them as virtual modules.

Supported MIME types:

- `text/css` → loaded as CSS module

- `text/javascript` → loaded as JS module

- `application/json` → loaded as JSON module

Other MIME types are ignored and fall back to default behavior.

## 🚀 Debug Usage

This plugin is enabled by default in `rolldown`, so no manual configuration is required.

```js
import msg from 'data:text/javascript,export default "hello from data URI"';
console.log(msg); // -> hello from data URI
```

If you want to specify the type of module, you can define `'module_ext': module_type`. For example, if you want to import files with ext `.png` as dataurl module.

```js
import { defineConfig } from 'rolldown';

export default defineConfig({
  input: './index.js',
  moduleTypes: {
    '.png': 'dataurl',
  },
});
```

It will yield

```js
// #region example.png
var example_default$12 = 'data:image/png,png';

// #endregion
```
