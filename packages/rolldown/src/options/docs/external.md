The matched IDs should be either:

1. the name of an external dependency, exactly the way it is written in the import statement. I.e. to mark `import "dependency.js"` as external, use `"dependency.js"` while to mark `import "dependency"` as external, use `"dependency"`.
1. a resolved ID (like an absolute path to a file).

When creating an `iife` or `umd` bundle, you will need to provide global variable names to replace your external imports via the [`output.globals`](/reference/OutputOptions.globals) option.

#### Examples

##### String pattern

```js
export default {
  external: 'react',
};
```

##### Regular expression

```js
export default {
  external: /node_modules/,
};
```

##### Array of patterns

```js
export default {
  external: ['react', 'react-dom', /^lodash/],
};
```

#### In-depth

##### ⚠️ Don't use function unless you have to

Using the function form has significant performance overhead because Rolldown is written in Rust and must call JavaScript functions from Rust for every module in your dependency graph.

**Performance Impact:**

- Each module triggers a Rust-to-JS call
- Cross-language call overhead is high
- Can significantly slow down builds in large projects

**Use static patterns when possible:**

```js
// ❌ Avoid: Function with performance overhead
export default {
  external: (id) => {
    return !id.startsWith('.') && !id.startsWith('/');
  },
};

// ✅ Prefer: Static pattern (much faster)
export default {
  external: [
    'react',
    'react-dom',
    'vue',
    /^lodash/,
    /^@mui/,
  ],
};
```

**When to use function:**

- You need truly dynamic logic based on `parentId` or `isResolved`
- The logic cannot be expressed with static patterns
- You're okay with the performance trade-off

##### ⚠️ Don't use `/node_modules/` to match npm packages

Using `/node_modules/` to externalize npm packages is problematic because Rolldown matches module IDs twice during resolution.

**Example with `import Vue from 'vue'`:**

1. **First match (unresolved ID):** `'vue'`
   - Pattern `/node_modules/` does NOT match
   - This is the bare package name

2. **Second match (resolved ID):** `'/path/to/project/node_modules/vue/dist/vue.runtime.esm-bundler.js'`
   - Pattern `/node_modules/` DOES match
   - This is the full resolved file path

**The Problem:**

Since the pattern only matches on the resolved ID, Rolldown generates imports with absolute paths:

```js
// ❌ Bad result: Absolute path in output
import Vue from '/Users/somebody/project/node_modules/vue/dist/vue.runtime.esm-bundler.js';
```

This breaks portability and doesn't work as intended.

**Better alternatives:**

- **Use exact package names**

```js
export default defineConfig({
  external: ['vue', 'react', 'react-dom'],
});
```

- **Use package name patterns**

```js
export default {
  external: [/^vue/, /^react/, /^@mui/],
};
```

- **Match bare identifiers**

Pattern ([visualize](https://regex-vis.com/?r=%5E%5B%5E.%2F%5D)) to match all bare module IDs (not starting with `.` or `/`):

```js
export default {
  external: /^[^./]/,
};
```
