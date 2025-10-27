# Optimization Options

- **Type:** `object`

Configure optimization features for the bundler.

## inlineConst

- **Type:** `boolean | { mode?: 'all' | 'smart'; pass?: number }`
- **Default:** `false`
- **Path:** `optimization.inlineConst`

Inline imported constant values during bundling instead of preserving variable references.

When enabled, constant values from imported modules will be inlined at their usage sites, potentially reducing bundle size and improving runtime performance by eliminating variable lookups.

**Options:**

- `true`: Equivalent to `{ mode: 'all', pass: 1 }`, enabling constant inlining for all eligible constants with a single pass.
- `false`: Disable constant inlining (default).
- `{ mode: 'smart' | 'all', pass?: number }`:
  - `mode: 'all'`: Inline all imported constants wherever they are used. Constant declarations are removed from the output.
  - `mode: 'smart'`: Selective inlining that balances bundle size optimization with readability. Constants are kept as variable declarations in the output, but inlined in two scenarios:
    1. **Specific expression contexts** where inlining enables dead code elimination:
       - `if (test) {}` - test expressions in if statements
       - `test ? a : b` - test expressions in ternary operators
       - `test1 || test2` - logical OR expressions
       - `test1 && test2` - logical AND expressions
       - `test1 ?? test2` - nullish coalescing expressions
    2. **Small constants** that are always safe to inline (won't increase bundle size):
       - Booleans: `true`, `false`
       - Null and undefined: `null`, `undefined`
       - Integer numbers between -99 and 999
       - Strings with 3 or fewer characters
  - `pass`: Number of passes to perform for constant inlining (default: `1`). Higher values enable multi-level constant propagation across module boundaries.

**Examples:**

**Input:**
:::code-group

```js [entry.js]
import { FLAG, MODE, SHORT } from './constants.js';

console.log(MODE);
if (MODE === 'production') {
  console.log('Production mode');
}
console.log(FLAG);
console.log(SHORT);
```

```js [constants.js]
export const MODE = 'production';
export const FLAG = true;
export const SHORT = 'dev';
```

:::

**Output with `{ mode: 'smart' }`:**

```js
// #region index.js
console.log(MODE); // Not inlined (not in special context, string > 3 chars)
if ('production' === 'production') console.log('Production mode');
console.log(true);
console.log('dev');
```

**Output with `{ mode: 'all' }`:**

```js
console.log('production');
if ('production' === 'production') {
  console.log('Production mode');
}
console.log(true);
console.log('dev');
```
