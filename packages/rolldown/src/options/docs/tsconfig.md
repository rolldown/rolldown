#### Options

##### Auto-discovery mode (`true`)

When set to `true`, Rolldown enables auto-discovery mode. For each module, both the resolver and transformer search upward from the module's directory for a `tsconfig.json` that **owns the file**, the way TypeScript selects a project. A `tsconfig.json` whose `files`/`include`/`exclude` (or one of its `references`) does not cover the file is skipped, and the search continues in the parent directory. If nothing up the tree owns the file, Rolldown falls back to the **outermost (topmost)** `tsconfig.json` found, not the nearest one.

If the tsconfig has `references`, Rolldown resolves them the way TypeScript does: a referenced project that includes the file **takes precedence over the root**, and the first matching reference wins. Each referenced project uses its own `allowJs`, so a `.js`/`.jsx`/`.mjs`/`.cjs` file is only included by projects that enable it. If no referenced project includes the file, Rolldown falls back to the root's own `files`/`include`/`exclude`. A solution-style root (only `references` with an explicit empty `files`/`include`, as Vite scaffolds) includes nothing on its own, so once none of its references match either, it does **not** claim the file, and discovery continues in the parent directories as described above.

```js
export default {
  tsconfig: true,
};
```

::: warning Behavior change in v1.1.0

Rolldown 1.1.0 realigned tsconfig resolution with TypeScript. Discovery now walks **up** through ancestor directories and uses the `tsconfig.json` that actually owns the file, instead of stopping at the nearest one and falling back to the topmost when nothing owns it. For a tsconfig with `references`, a referenced project that includes the file takes precedence over the root, and each referenced project's own `allowJs` decides whether `.js`/`.jsx`/`.mjs`/`.cjs` files are included.

For most projects this is a fix, but it is a breaking change if you relied on the old "nearest tsconfig / root wins" behavior. See the [v1.1.0 release notes](https://github.com/rolldown/rolldown/releases/tag/v1.1.0) for migration guidance.

:::

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
