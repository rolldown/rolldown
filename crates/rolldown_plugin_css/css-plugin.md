# `rolldown_plugin_css` Design Document

## Overview

A standalone CSS bundling plugin for Rolldown, implemented entirely in Rust as a builtin plugin. It handles CSS extraction, code splitting, `@import` inlining, `url()` rewriting, CSS modules, minification, preprocessor compilation, source maps, and single-bundle mode — with no core bundler changes required.

Inspired by Vite's CSS pipeline (`vite:css` + `vite:css-post`) but stripped of all Vite-specific concerns (HMR, dev server, environment API, public dir). Uses **lightningcss** as the CSS engine.

**Crate:** `crates/rolldown_plugin_css/`
**Plugin name:** `builtin:css`
**NAPI binding:** `BindingBuiltinPluginName::Css`
**JS export:** `cssPlugin()` from `rolldown/experimental`

## Architecture

```
build_start   → Initialize shared state caches in PluginContextMeta
transform     → Preprocess → inline @imports → detect CSS modules → cache CSS → emit JS proxy
render_chunk  → Collect CSS per chunk → extract url() deps → emit assets → finalize → emit .css
augment_chunk_hash → Include CSS content in chunk hashing
generate_bundle    → Prune pure CSS JS chunks → emit single bundle (if !code_split)
```

The plugin intercepts `ModuleType::Css` modules (and preprocessor extensions) in `transform`, replaces them with JS proxy modules, caches the CSS content, then reassembles and emits CSS assets during the generate phase.

## Configuration

```rust
pub struct CssPluginOptions {
    pub code_split: bool,  // Per-chunk .css files (true) or single bundle (false)
    pub minify: bool,      // Minify via lightningcss
    pub sourcemap: bool,   // Generate .css.map files
}
```

```ts
import { cssPlugin } from 'rolldown/experimental';

export default {
  plugins: [cssPlugin({ codeSplit: true, minify: true, sourcemap: false })],
};
```

## Key Design Decisions

### 1. Module type as the contract, not file extensions

The plugin checks `args.module_type == ModuleType::Css`, not file extensions. This is the contract — other plugins can set a module's type to `Css` to route it through this pipeline. Preprocessor files (.scss, .less, .styl) are detected by extension since they arrive with a different module type.

### 2. CSS → JS proxy replacement via `module_type: Js`

The `transform` hook returns `module_type: Some(ModuleType::Js)`, which tells Rolldown to parse the returned code as JavaScript instead of CSS. The original CSS is preserved in `CssStylesCache`. This lets the module graph track CSS dependencies through JS import edges while keeping the actual CSS content separate.

### 3. Side effects: `NoTreeshake` vs `False`

- **Plain CSS** → `HookSideEffects::NoTreeshake`: The empty JS proxy would be tree-shaken away (no exports, no detectable effects), which would lose the CSS. `NoTreeshake` keeps the module in the graph unconditionally.
- **CSS modules** → `HookSideEffects::False`: CSS modules have real JS exports. If nothing imports them, dropping both the JS and CSS is correct. If they are consumed, the module stays naturally.

This matches Vite's approach (`moduleSideEffects: inlined ? false : 'no-treeshake'`).

### 4. Shared state via `PluginContextMeta`

All inter-hook state is stored as type-erased `Arc<T>` values in `PluginContextMeta` (keyed by `TypeId`). No global state. Types:

| Type              | Purpose                                                                            |
| ----------------- | ---------------------------------------------------------------------------------- |
| `CssStylesCache`  | `FxDashMap<String, String>` — module ID → CSS content                              |
| `PureCssChunks`   | `FxDashSet<ArcStr>` — filenames of chunks containing only CSS                      |
| `ChunkCssMap`     | `FxDashMap<ArcStr, ArcStr>` — chunk filename → emitted CSS reference ID            |
| `AccumulatedCss`  | `Mutex<Vec<(ArcStr, String)>>` — ordered (chunk, css) pairs for single-bundle mode |
| `UrlPlaceholders` | `FxDashMap<String, ArcStr>` — lightningcss placeholder → emitted asset reference   |

### 5. CSS ordering follows module execution order

CSS within a chunk is ordered by `chunk.module_ids` (which reflects Rolldown's execution order). In single-bundle mode, chunks are traversed: entry chunks first (static imports depth-first), then dynamic imports. This ensures CSS specificity is correct — earlier-loaded CSS has lower priority.

### 6. Finalization pipeline

Every CSS output goes through `finalize_css()` before emission:

```
url() placeholder replacement → @charset/@import hoisting → minification (optional)
```

- **url() replacement**: Lightningcss replaces `url()` values with deterministic placeholders during `analyze_dependencies`. The plugin resolves these to output-relative paths after asset emission.
- **At-rule hoisting**: Per CSS spec, `@charset` must be first, `@import` before other rules. Concatenation can violate this — hoisting fixes it. Duplicate `@charset` rules are deduplicated.
- **Minification**: lightningcss re-parses and re-serializes with `minify: true`.

### 7. Pure CSS chunk cleanup

Chunks where every module is CSS and `exports` is empty are "pure CSS chunks" — they exist only as vehicles for CSS side-effects. In `generate_bundle`:

1. These JS chunks are removed from the bundle
2. Import statements referencing them in other chunks are replaced with `/* empty css */`
3. Associated `.js.map` files are also removed

### 8. @import inlining via lightningcss bundler

Uses lightningcss's `Bundler` + `SourceProvider` trait rather than regex-based parsing. This correctly handles:

- `@layer` wrapping of imported rules
- `@media`/`@supports` condition wrapping
- Circular import detection
- CSS module `composes` dependencies

The `SourceProvider` resolves specifiers relative to the originating file's directory with `.css` extension fallback. All resolved paths are tracked for watch mode via `ctx.add_watch_file()`.

**Fast path**: If the CSS contains no `@import`, the bundler is skipped entirely.

### 9. CSS modules use `[hash]_[local]` pattern

lightningcss's native CSS modules support generates scoped class names. The JS proxy exports both named exports and a default export object:

```js
export const container = 'x7y8z9_container';
export const title = 'a1b2c3_title';
export default {
  container: 'x7y8z9_container',
  title: 'a1b2c3_title',
};
```

`composes` references (Local, Global, Dependency) are concatenated with spaces: `"hash_name composed1 composed2"`. Exports are sorted alphabetically for deterministic output. Hyphenated class names are sanitized to valid JS identifiers (`foo-bar` → `foo_bar`).

### 10. Source maps bridge `parcel_sourcemap` → `oxc_sourcemap`

lightningcss uses `parcel_sourcemap::SourceMap` internally, while Rolldown uses `oxc_sourcemap::SourceMap`. The bridge is JSON serialization (`sm.to_json()` → `SourceMap::from_json_string()`). Source maps are:

1. Generated per-module via `parse_css_with_sourcemap()`
2. Joined via `SourceJoiner` (adjusts line offsets for concatenation)
3. Chained with minification maps via `collapse_sourcemaps()`
4. Emitted as `.css.map` files with `/*# sourceMappingURL=... */` comments

### 11. Preprocessor support: Sass native, Less/Stylus stubbed

- **Sass/SCSS**: Compiled via the `grass` crate (pure Rust). Load paths set to the file's parent directory.
- **Less/Stylus**: Pass-through stubs. The source is returned as-is, which works for the subset of Less/Stylus that is valid CSS. Full compilation would require Node.js-based compilers.
- Detection is by file extension, checked **before** module type (preprocessor files may not arrive as `ModuleType::Css`).
- `.module.scss`, `.module.less` etc. are handled (preprocessor + CSS modules).

### 12. url() rewriting skips non-local URLs

Skipped (left as-is): `data:` URLs, `http://`, `https://`, `//` (protocol-relative). Fragment identifiers (`#icon`) are preserved and re-appended after path resolution. Missing assets are silently skipped (placeholder remains) rather than failing the build.

## File Structure

```
crates/rolldown_plugin_css/
├── Cargo.toml
├── css-plugin.md              ← this document
└── src/
    ├── lib.rs                 ← Plugin struct, options, hooks, shared state types
    ├── import_inlining.rs     ← @import resolution via lightningcss Bundler + SourceProvider
    ├── url_rewriting.rs       ← url() extraction, asset emission, placeholder replacement
    ├── css_modules.rs         ← CSS modules: scoped names, JS proxy generation
    ├── minification.rs        ← CSS minification via lightningcss
    ├── at_rule_hoisting.rs    ← @charset/@import hoisting per CSS spec
    ├── generate_bundle.rs     ← Pure CSS chunk cleanup + single-bundle emission
    ├── preprocessors.rs       ← Sass (grass), Less/Stylus stubs
    └── sourcemap.rs           ← parcel↔oxc bridge, joining, minification maps
```

## Vite-specific logic deliberately excluded

| Feature                                        | Reason                        |
| ---------------------------------------------- | ----------------------------- |
| HMR (`updateStyle`/`removeStyle`)              | Dev server concern            |
| `?html-proxy` for `<style>`                    | Vite HTML plugin concern      |
| `?direct` / `?inline` / `?url` queries         | Dev server / consumer concern |
| `checkPublicFile()` / `publicFileToBuiltUrl()` | Vite public dir handling      |
| `import.meta.hot`                              | HMR API                       |
| `viteMetadata.importedCss`                     | Vite chunk metadata           |
| `cssScopeTo`                                   | Vite-specific CSS scoping     |
| Legacy build handling                          | Vite legacy plugin            |
| PostCSS integration                            | Can be a separate plugin      |

## Testing

Test fixtures use `_config.json` with `"plugin": { "css": { ... } }` to declaratively enable the plugin:

```json
{
  "config": { "moduleTypes": { ".css": "css" } },
  "plugin": { "css": { "codeSplit": true } }
}
```

The `PluginTestMeta` struct in `rolldown_testing_config` deserializes this and constructs a `CssPlugin` instance, inserted first in the plugins vec so it runs before other plugins.

Fixtures: `crates/rolldown/tests/rolldown/topics/css/{.basic,.align_vite,.css_entries}/`

## Dependencies

| Crate                                   | Purpose                                                              |
| --------------------------------------- | -------------------------------------------------------------------- |
| `lightningcss` (with `bundler` feature) | CSS parsing, modules, @import bundling, minification, url() analysis |
| `grass`                                 | Sass/SCSS compilation (pure Rust)                                    |
| `parcel_sourcemap`                      | Source map bridge (lightningcss output format)                       |
| `rolldown_sourcemap`                    | Source map joining and collapsing                                    |
| `rolldown_plugin_utils`                 | `is_css_module()` helper                                             |
| `sugar_path`                            | Relative path computation for url() rewriting                        |
