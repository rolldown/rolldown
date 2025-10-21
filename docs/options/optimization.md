# Optimization Options

- **Type:** `object`
- **Default:** `{}`

Configure optimization features for the bundler.

## inlineConst

- **Type:** `boolean | { mode?: 'all' | 'smart'; pass?: number }`
- **Default:** `false`
- **Path:** `optimization.inlineConst`

Inline imported constant values during bundling instead of preserving variable references. When enabled, constant values from imported modules will be inlined at their usage sites, potentially reducing bundle size and improving runtime performance by eliminating variable lookups.
