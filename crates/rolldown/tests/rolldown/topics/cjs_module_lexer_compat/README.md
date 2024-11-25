This folder contains tests for compatibility with the `cjs-module-lexer` package.

We want to ensure our cjs output is compatible `cjs-module-lexer`. Reasons are

- Node.js uses `cjs-module-lexer` to parse CommonJS modules when using static `import` cjs modules.
- Detected exports could be imported via [named imports](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/import#named_import).
- We want rolldown's cjs output to be friendly to Node.js.
