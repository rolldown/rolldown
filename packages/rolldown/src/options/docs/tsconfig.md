#### Options

##### Auto-discovery mode (`true`)

When set to `true`, Rolldown enables auto-discovery mode (similar to Vite). For each module, both the resolver and transformer will find the nearest `tsconfig.json`.

If the tsconfig has `references`, Rolldown resolves them the way TypeScript does: each referenced project is checked in order, and the **first one that includes the file takes precedence over the root**. A referenced project includes the file when the file's extension is allowed by that project's own `allowJs` and the file matches its `files`/`include` patterns (and is not excluded). Only when no referenced project includes the file does Rolldown fall back to the root tsconfig.

:::info Changed in v1.1.0
Project references are now resolved like TypeScript: they are checked before the root tsconfig, so the `paths` / `allowJs` that apply to a file may differ from earlier versions.
:::

```js
export default {
  tsconfig: true,
};
```

##### Explicit path (`string`)

Specifies the path to a specific TypeScript configuration file. You may provide a relative path (resolved relative to `cwd`) or an absolute path.

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

:::tip
Rolldown respects `references` and `include`/`exclude` patterns in tsconfig, while esbuild does not. If you need esbuild-compatible behavior, specify a tsconfig without `references`. You can use [`extends`](https://www.typescriptlang.org/tsconfig/#extends) to share the options between the two.
:::

#### What's used from tsconfig

When a tsconfig is resolved, Rolldown uses different parts for different purposes:

##### Resolver

Uses the following for module path mapping:

- `compilerOptions.paths`: Path mapping for module resolution
- `compilerOptions.baseUrl`: Base directory for path resolution

##### Transformer

Uses select compiler options including:

- `jsx`: JSX transformation mode
- `experimentalDecorators`: Enable decorator support
- `emitDecoratorMetadata`: Emit decorator metadata
- `verbatimModuleSyntax`: Module syntax preservation
- `useDefineForClassFields`: Class field semantics
- And other TypeScript-specific options

##### Example

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

#### Priority

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

:::tip
For TypeScript projects, it's recommended to use `tsconfig: true` for auto-discovery or specify an explicit path to ensure consistent compilation behavior and enable path mapping.
:::
