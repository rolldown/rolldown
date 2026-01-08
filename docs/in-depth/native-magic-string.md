# Native MagicString

## Overview

`experimental.nativeMagicString` is an optimization feature that replaces the JavaScript-based MagicString implementation with a native Rust version, enabling source map generation in background threads for improving performance.

## What is MagicString?

MagicString is a JavaScript library developed by Rich Harris (the creator of Rollup and Svelte) that provides efficient string manipulation with automatic source map generation. It's commonly used by bundlers and build tools for:

- Code transformation in plugins
- Source map generation
- Precise line/column tracking
- Efficient string operations (replace, prepend, append, etc.)

## The JavaScript Implementation vs Native Rust

### Traditional JavaScript MagicString

The original MagicString implementation is written in JavaScript and runs in the Node.js environment. When bundlers perform code transformations, they typically:

1. Load source code as JavaScript strings
2. Apply transformations using MagicString API
3. Generate source maps for the transformed code
4. Process everything in the main JavaScript thread

### Native Rust Implementation

Rolldown's native MagicString implementation rewrites the core functionality in Rust, providing several advantages:

- **Performance**: Rust's memory safety and zero-cost abstractions make string operations faster
- **Parallel Processing**: Source map generation can happen in background threads
- **Memory Efficiency**: Better memory management for large codebases
- **Integration**: Seamless integration with Rolldown's Rust-based architecture

## How It Works

When `experimental.nativeMagicString` is enabled, Rolldown modifies the transformation pipeline. The diagrams below show the architectural differences:

:::info
Some technical details are simplified for better illustration. The native MagicString implementation provides a `magicString` object in the `meta` parameter of transform hooks, which plugins can use just like the JavaScript version.
:::

### Without Native MagicString

<img width="3426" height="1699" alt="js-magic-string" src="https://github.com/user-attachments/assets/c9e81f8a-fad0-4f99-99c4-c71c67b8912e" style="background: white;" />

(Correction in the image: "rolldown without js magic-string" should be "rolldown without native magic-string")

### With Native MagicString

<img width="3343" height="1659" alt="native-magic-string" src="https://github.com/user-attachments/assets/71ca5d7b-9b40-46ce-86dd-bfa4bdd73f4b" style="background: white;" />

**Key Difference**: The native implementation is written in Rust, providing both Rust's performance advantages and background thread source map generation. Offloading to background threads improves overall CPU usage and enables significant performance improvements.

## API Compatibility

The native implementation maintains API compatibility with the JavaScript version. The most commonly used APIs are already implemented, with the remaining APIs planned for completion in future releases.

### Implemented Methods

The following MagicString methods are currently available in the native implementation:

**String Manipulation:**

- `append(content)` - Appends content to the end of the string
- `prepend(content)` - Prepends content to the beginning of the string
- `appendLeft(index, content)` - Appends content to the left of a specific index
- `appendRight(index, content)` - Appends content to the right of a specific index
- `prependLeft(index, content)` - Prepends content to the left of a specific index
- `prependRight(index, content)` - Prepends content to the right of a specific index
- `overwrite(start, end, content)` - Replaces content in a range
- `update(start, end, content)` - Updates content in a range
- `remove(start, end)` - Removes content in a range
- `replace(from, to)` - Replaces the first occurrence of a pattern
- `replaceAll(from, to)` - Replaces all occurrences of a pattern

**Transformations:**

- `indent(indentor?)` - Indents the content with optional custom indentation string
- `relocate(start, end, to)` - Moves content from one position to another

**Utilities:**

- `toString()` - Returns the transformed string
- `hasChanged()` - Checks if the string has been modified
- `length()` - Returns the length of the transformed string
- `isEmpty()` - Checks if the string is empty

### Not Yet Implemented

The following features are planned for future releases:

- Advanced source map options (`generateMap()`, `generateDecodedMap()`)
- `clone()` method
- `trim()` / `trimStart()` / `trimEnd()` methods
- `snip()` method

## Real-World Performance

use [rolldown/benchmarks](https://github.com/rolldown/benchmarks/) as benchmark cases

### Build time

| Runs       | oxc raw transfer + js magicString | oxc raw transfer + native magicString | Time Saved | Speedup |
| ---------- | --------------------------------- | ------------------------------------- | ---------- | ------- |
| apps/1000  | 497.6 ms                          | 431.1 ms                              | 66.5 ms    | 1.15x   |
| apps/5000  | 1.100 s                           | 894.5 ms                              | 205.5 ms   | 1.23x   |
| apps/10000 | 1.814 s                           | 1.368 s                               | 446.0 ms   | 1.33x   |

### Plugin transform time (build time - noop plugin build time)

| Runs  | Transform Time (oxc raw transfer + js magicString) | Transform Time (oxc raw transfer + native magicString) | Time Saved | Speedup |
| ----- | -------------------------------------------------- | ------------------------------------------------------ | ---------- | ------- |
| 1000  | 172.0 ms                                           | 105.5 ms                                               | 66.5 ms    | 1.63x   |
| 5000  | 455.4 ms                                           | 249.9 ms                                               | 205.5 ms   | 1.82x   |
| 10000 | 799.0 ms                                           | 353.0 ms                                               | 446.0 ms   | 2.26x   |

For detailed benchmark results, see the [benchmark pull request](https://github.com/rolldown/benchmarks/pull/9/files).

## Usage Examples

### Basic Plugin with Native MagicString

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';

export default defineConfig({
  experimental: {
    nativeMagicString: true,
  },
  output: {
    sourcemap: true,
  },
  plugins: [
    {
      name: 'transform-example',
      transform(code, id, meta) {
        if (!meta?.magicString) {
          // Fallback when nativeMagicString is not available
          return null;
        }

        const { magicString } = meta;

        // Example transformation: Add debug comments
        if (code.includes('console.log')) {
          magicString.replace(/console\.log\(/g, 'console.log("[DEBUG]", ');
        }

        // Example: Add file header
        magicString.prepend(`// Transformed from: ${id}\n`);

        return {
          code: magicString,
        };
      },
    },
  ],
});
```

## Compatibility and Fallbacks

### Checking for Native MagicString Availability

```javascript [rolldown.config.js]
transform(code, id, meta) {
  if (meta?.magicString) {
    // Native MagicString is available
    const { magicString } = meta;

    // Use the native implementation
    // Note: Return the magicString object directly, not a string
    return {
      code: magicString
    };
  } else {
    // Fallback to regular string manipulation
    // or use the JavaScript MagicString library
    const MagicString = require('magic-string');
    const ms = new MagicString(code);

    // Your transformations here...

    return {
      code: ms.toString(),
      map: ms.generateMap()
    };
  }
}
```

### Rollup Compatibility

This feature is Rolldown-specific and not available in Rollup. For plugins that need to work with both bundlers:

```javascript [plugin.js]
function createTransform() {
  return function (code, id, meta) {
    if (meta?.magicString) {
      // Rolldown with native MagicString
      return transformWithNativeMagicString(code, id, meta);
    } else {
      // Rollup or Rolldown without native MagicString
      return transformWithJsMagicString(code, id);
    }
  };
}
```

::: tip

You can use [`rolldown-string`](https://github.com/sxzz/rolldown-string), which provides a unified interface that works with both bundlers.

:::

## When to Use Native MagicString

### Recommended Scenarios

1. **Large Codebases**: Projects with hundreds or thousands of files
2. **Complex Transformations**: Plugins that perform extensive code manipulation
3. **Source Map Intensive**: Projects requiring detailed source maps
4. **Performance-Critical**: Build processes where speed is crucial
5. **Development Mode**: Faster rebuild times during development

### When to Be Cautious

1. **Experimental Feature**: As an experimental feature, API may change
2. **Plugin Compatibility**: Some plugins may expect specific JavaScript MagicString behavior
3. **Debugging**: Native implementation may have different error messages

## Migration Guide

### Enabling Native MagicString

1. **Update Configuration**:

```javascript [rolldown.config.js]
export default {
  experimental: {
    nativeMagicString: true,
  },
  output: {
    sourcemap: true, // Required for source map generation
  },
};
```

2. **Update Plugins**:

```javascript [rolldown.config.js]
// Before
transform(code, id) {
  const ms = new MagicString(code);
  // ... transformations
  return { code: ms.toString(), map: ms.generateMap() };
}

// After
transform(code, id, meta) {
  if (meta?.magicString) {
    const { magicString } = meta;
    // ... transformations (same API)
    return { code: magicString };
  }
  // Fallback logic
}
```

## Limitations and Considerations

### Current Limitations

1. **Experimental Status**: API may change in future versions
2. **Edge Cases**: Some edge cases may behave differently from JavaScript version
3. **Debugging**: Error messages may be less familiar

### Best Practices

1. **Always Check Availability**: Verify `meta?.magicString` exists before using
2. **Provide Fallbacks**: Include fallback logic for compatibility
3. **Test Thoroughly**: Test transformations with both implementations
4. **Report Issues**: Report any behavior differences to the Rolldown team

## Conclusion

`experimental.nativeMagicString` represents a significant performance optimization for Rolldown by leveraging Rust's efficiency for code transformation tasks. While it requires some considerations for compatibility, the performance benefits make it an attractive option for large-scale projects and performance-critical build processes.

As an experimental feature, it's recommended to test thoroughly in development environments before adopting in production workflows. The Rolldown team is actively working on this feature, and feedback from the community is valuable for its continued development.
