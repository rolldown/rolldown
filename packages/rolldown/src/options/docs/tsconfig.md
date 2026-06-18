#### Options

##### Auto-discovery mode (`true`)

When set to `true`, Rolldown enables auto-discovery mode. For each module, both the resolver and transformer search **upward** from the module's directory, starting at the nearest `tsconfig.json`. If it has `references`, Rolldown checks each referenced project's `files`/`include`/`exclude` and uses the first one that matches the file. If no reference matches, it checks the `tsconfig.json`'s own `files`/`include`/`exclude`. If the file matches neither, Rolldown continues upward to the next `tsconfig.json` and repeats. If no `tsconfig.json` owns the file, no config is applied (no `paths`/`baseUrl`), matching TypeScript.

Whether a `files`/`include` entry matches a file depends on its extension: by default only TypeScript files (`.ts`/`.tsx`/`.mts`/`.cts`) match, plus `.js`/`.jsx`/`.mjs`/`.cjs` when `allowJs` is enabled. An entry that names an explicit extension (for example `src/**/*.vue`) matches that extension verbatim, so a non-TS file can be owned by the project and pick up its `paths`/`baseUrl`.

If the tsconfig has `references`, Rolldown resolves them the way TypeScript does: a referenced project that includes the file **takes precedence over the root**, and the first matching reference wins. Each referenced project matches with its own `compilerOptions` (such as `allowJs`). If no referenced project includes the file, Rolldown falls back to the root's own `files`/`include`/`exclude`. A solution-style root (only `references` with an explicit empty `files`/`include`, as Vite scaffolds) has no file patterns of its own, so once none of its references match either, it does **not** own the file, and discovery continues in the parent directories as described above.

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
- `strictNullChecks` (falling back to `strict`): Controls whether `null`/`undefined` are elided from nullable-union `design:type` decorator metadata, and only applies when `emitDecoratorMetadata` is enabled. When neither is set it defaults to enabled, matching TypeScript 6.0+ (where `strict` is on by default)
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
