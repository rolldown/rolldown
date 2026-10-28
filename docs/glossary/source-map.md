# Source Map

A **source map** maps positions in bundled (and often minified) output back to the original source files. Debuggers and browser DevTools use source maps so stack traces and breakpoints refer to your TypeScript or unminified JavaScript instead of the generated chunk.

Enable them with [`output.sourcemap`](/reference/OutputOptions.sourcemap) (for example `true`, `'inline'`, or `'hidden'`).

Plugins that transform code should return a map for their edits (or an empty `mappings` string when a map is not meaningful). See [Source Code Transformations](/apis/plugin-api/transformations).
