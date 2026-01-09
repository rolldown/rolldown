# tsconfig

- **Type:** `boolean | string`
- **Optional:** Yes ✅
- **Default:** `true`

Configures TypeScript configuration file resolution and usage.

:::warning
In monorepo projects, if a sub-package does not have its own `tsconfig.json`, auto-discovery may unexpectedly match the root `tsconfig.json`. This can cause unintended compilation behavior (e.g., different `paths` mappings or compiler options). To avoid this, either:

- Create a `tsconfig.json` in each sub-package
- Use `tsconfig: false` to disable tsconfig resolution
- Explicitly specify `tsconfig: './path/to/tsconfig.json'`
  :::

## Options

### Auto-discovery mode (`true`)

When set to `true`, Rolldown enables auto-discovery mode (similar to Vite). For each module, both the resolver and transformer will find the nearest `tsconfig.json`.

If the tsconfig has `references` and certain conditions are met (the file extension is allowed and the tsconfig's `include`/`exclude` patterns don't match the file), then the referenced tsconfigs will be searched for a match. If no match is found, it falls back to the original tsconfig.

### Explicit path (`string`)

Specifies the path to a specific TypeScript configuration file. You may provide a relative path (resolved relative to [`cwd`](./cwd.md)) or an absolute path.

If the tsconfig has `references`, this mode behaves like auto-discovery mode for reference resolution.

```js
export default {
  tsconfig: './tsconfig.json',
};
```

```js
export default {
  tsconfig: '/absolute/path/to/tsconfig.json',
};
```

:::tip Note
Rolldown respects `references` and `include`/`exclude` patterns in tsconfig, while esbuild does not. If you need esbuild-compatible behavior, specify a tsconfig without `references`. You can use [`extends`](https://www.typescriptlang.org/tsconfig/#extends) to share the options between the two.
:::

## What's used from tsconfig

When a tsconfig is resolved, Rolldown uses different parts for different purposes:

### Resolver

Uses the following for module path mapping:

- `compilerOptions.paths`: Path mapping for module resolution
- `compilerOptions.baseUrl`: Base directory for path resolution

### Transformer

Uses select compiler options including:

- `jsx`: JSX transformation mode
- `experimentalDecorators`: Enable decorator support
- `emitDecoratorMetadata`: Emit decorator metadata
- `verbatimModuleSyntax`: Module syntax preservation
- `useDefineForClassFields`: Class field semantics
- And other TypeScript-specific options

### Example

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "jsx": "react-jsx",
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"],
      "@components/*": ["src/components/*"]
    }
  }
}
```

With this configuration:

- JSX will use React's automatic runtime
- Path aliases like `@/utils` will resolve to `src/utils`

## Priority

Top-level `transform` options always take precedence over tsconfig settings:

```js
export default {
  tsconfig: './tsconfig.json', // Has jsx: 'react-jsx'
  transform: {
    jsx: {
      mode: 'classic', // This takes precedence
    },
  },
};
```
