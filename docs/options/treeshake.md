# treeshake

- **Type:** `boolean | TreeshakingOptions`
- **Default:** `true`

Controls tree-shaking (dead code elimination). When `true`, unused code will be removed from the bundle to reduce bundle size.

## TreeshakingOptions

When passing an object, you can fine-tune the tree-shaking behavior with the following options:

### treeshake.moduleSideEffects

- **Type:** `boolean | readonly string[] | ModuleSideEffectsRule[] | ((id: string, external: boolean) => boolean | undefined) | 'no-external'`
- **Default:** `true`

Controls whether imported modules have side effects. This option helps tree-shaking determine which modules must be included even if their exports aren't used, and which can be safely removed.

**Values:**

- **`true`**: All modules are assumed to have side effects and will be included in the bundle even if none of their exports are used.

- **`false`**: No modules have side effects. This enables aggressive tree-shaking, removing any modules whose exports are not used.

- **`string[]`**: Array of module IDs that have side effects. Only modules in this list will be preserved if unused; all others can be tree-shaken when their exports are unused.

- **`'no-external'`**: Assumes no external modules have side effects while preserving the default behavior for local modules.

- **`ModuleSideEffectsRule[]`**: Array of rules with `test`, `external`, and `sideEffects` properties for fine-grained control.

- **`function`**: Function that receives `(id, external)` and returns whether the module has side effects.

**Important:** Setting this to `false` or using an array/string assumes that your modules and their dependencies have no side effects other than their exports. Only use this if you're certain that removing unused modules won't break your application.

> [!NOTE]
> **Performance: Prefer `ModuleSideEffectsRule[]` over functions**
>
> When possible, use rule-based configuration instead of functions. Rules are processed entirely in Rust, while JavaScript functions require runtime calls between Rust and JavaScript, which can hurt CPU utilization during builds.
>
> **Functions should be a last resort**: Only use the function signature when your logic cannot be expressed with patterns or simple string matching.
>
> **Rule advantages**: `ModuleSideEffectsRule[]` provides better performance by avoiding Rust-JavaScript runtime calls, clearer intent, and easier maintenance.

**Examples:**

```js
// Assume no modules have side effects (aggressive tree-shaking)
treeshake: {
  moduleSideEffects: false
}

// Only specific modules have side effects (string array)
treeshake: {
  moduleSideEffects: [
    'lodash',
    'react-dom',
  ];
}

// Use rules for pattern matching and granular control
treeshake: {
  moduleSideEffects: [
    { test: /^node:/, sideEffects: true },
    { test: /\.css$/, sideEffects: true },
    { test: /some-package/, sideEffects: false, external: false },
  ];
}

// Custom function to determine side effects
treeshake: {
  moduleSideEffects: ((id, external) => {
    if (external) return false; // external modules have no side effects
    return id.includes('/side-effects/') || id.endsWith('.css');
  });
}

// Assume no external modules have side effects
treeshake: {
  moduleSideEffects: 'no-external',
}
```

**Common Use Cases:**

- **CSS files**: `{ test: /\.css$/, sideEffects: true }` - preserve CSS imports
- **Polyfills**: Add specific polyfill modules to the array
- **Plugins**: Modules that register themselves globally on import
- **Library development**: Set to `false` for libraries where unused exports should be removed

### treeshake.annotations

- **Type:** `boolean`
- **Default:** `true`

Whether to respect `/*@__PURE__*/` annotations and other tree-shaking hints in the code.

### treeshake.manualPureFunctions

- **Type:** `readonly string[]`
- **Default:** `[]`

Array of function names that should be considered pure (no side effects) even if they can't be automatically detected as pure.

**Example:**

```js
treeshake: {
  manualPureFunctions: ['console.log', 'debug.trace'];
}
```

### treeshake.unknownGlobalSideEffects

- **Type:** `boolean`
- **Default:** `true`

Whether to assume that accessing unknown global properties might have side effects.

### treeshake.commonjs

- **Type:** `boolean`
- **Default:** `true`

Whether to enable tree-shaking for CommonJS modules. When `true`, unused exports from CommonJS modules can be eliminated from the bundle, similar to ES modules. When disabled, CommonJS modules will always be included in their entirety.

This option allows rolldown to analyze `exports.property` assignments in CommonJS modules and remove unused exports while preserving the module's side effects.

**Example:**

```js
// source.js (CommonJS)
exports.used = 'This will be kept';
exports.unused = 'This will be tree-shaken away';

// main.js
import { used } from './source.js';

// With commonjs: true, only the 'used' export is included in the bundle
// With commonjs: false, both exports are included
```

### treeshake.propertyReadSideEffects

- **Type:** `false | 'always'`
- **Default:** `false`

Controls whether reading properties from objects is considered to have side effects. Set to `'always'` for more conservative behavior.

### treeshake.propertyWriteSideEffects

- **Type:** `false | 'always'`
- **Default:** `'always'`

Controls whether writing properties to objects is considered to have side effects. Set to `'always'` for conservative behavior.
