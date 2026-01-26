:::warning

When using `output.banner` with minification enabled, the banner content may be stripped out unless it is formatted as a legal comment. To ensure your banner persists through minification, do either:

- Use [`output.postBanner`](/reference/OutputOptions.postBanner) instead, which are added after minification, or
- Use one of these comment formats:
  - Comments starting with `/*!` (e.g., `/*! My banner */`)
  - Comments containing `@license` (e.g., `/* @license My banner */`)
  - Comments containing `@preserve` (e.g., `/* @preserve My banner */`)
  - Comments starting with `//!` (for single-line comments)

The latter way's behavior is controlled by the [`output.legalComments`](/reference/OutputOptions.legalComments) option, which defaults to `'inline'` and preserves these special comment formats.

:::

#### Examples

##### Adding shebang for CLI tools

```js
export default {
  output: {
    banner: (chunk) => {
      // Add shebang only to the CLI entry point
      if (chunk.name === 'cli') {
        return '#!/usr/bin/env node';
      }
      return '';
    },
  },
};
```

##### Adding "use strict" directive

```js
export default {
  output: {
    format: 'cjs',
    banner: '"use strict";',
  },
};
```
