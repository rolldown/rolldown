# Native MagicString Example

This example demonstrates the use of `experimental.nativeMagicString` in Rolldown.

## What is nativeMagicString?

The `nativeMagicString` option enables a native Rust implementation of MagicString for source map generation and code transformation. This provides significant performance improvements over the JavaScript implementation, especially for large codebases.

## Key Features

- **Better Performance**: Native Rust implementation is faster than JavaScript
- **Background Processing**: Source map generation happens asynchronously in a background thread
- **Seamless Integration**: Works with existing plugins that use the MagicString API

## How It Works

When `experimental.nativeMagicString` is enabled:

1. Rolldown provides a `magicString` object in the `meta` parameter of transform hooks
2. Plugins can use this object to manipulate code (replace, prepend, append, etc.)
3. Source maps are automatically generated from the transformations
4. The native implementation handles the heavy lifting in a background thread

## Running This Example

```bash
npm run build
```

This will:

1. Bundle the source files
2. Apply transformations using the native MagicString implementation
3. Generate source maps
4. Output the bundled code to the `dist/` directory

## What the Plugin Does

The example plugin demonstrates three common MagicString operations:

1. **Replace**: Changes "Hello" to "Hi" in the code
2. **Prepend**: Adds a comment at the beginning of each file
3. **Append**: Adds a timestamp comment at the end of each file

All transformations maintain accurate source maps thanks to the native MagicString implementation.

## Configuration

```js
export default defineConfig({
  experimental: {
    nativeMagicString: true, // Enable native Rust implementation
  },
  output: {
    sourcemap: true, // Enable source map generation
  },
});
```

## Learn More

- [MagicString GitHub](https://github.com/rich-harris/magic-string)
- [Rolldown Documentation](https://rolldown.rs)
